//! Weiterleitung von HTTP-Anfragen an Plugins.
//!
//! Der Server stellt die Routen `/api/plugins/{name}` und
//! `/api/plugins/{name}/{*rest}` bereit. Der Host löst den Namen des Plugins
//! zur Anfragezeit auf und leitet Anfrage und Antwort weiter.
//!
//! Der Host bindet keine eigenen Ports. Das Plugin meldet seine Adresse in
//! der Antwort auf `initialize`. Der Host prüft die Adresse mit einem
//! URL-Parser. Zulässig sind nur `http` auf `127.0.0.1`, `localhost` und
//! `[::1]`. Nur dann wird weitergeleitet.
//!
//! Zugangsdaten des Hosts erreichen das Plugin nicht. Der Host entfernt vor
//! der Weiterleitung die Kopfzeilen `X-Admin-Token`, `Authorization` und
//! `Cookie`.

use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, Request, State};
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::any;
use edict_core::{AppError, AppResult};
use futures_util::StreamExt;
use reqwest::Client;
use url::Url;

use crate::registry::PluginRegistry;

/// Größte Rohkörpergröße, die der Host weiterleitet.
///
/// Der Wert begrenzt den Speicher, den eine einzelne Anfrage belegen darf.
pub const MAX_ROHKOERPER: usize = 8 * 1024 * 1024;

/// Kopfzeilen, die der Host nicht an das Plugin weitergibt.
const GESPERRTE_HEADER: [&str; 5] = [
    "host",
    "content-length",
    "x-admin-token",
    "authorization",
    "cookie",
];

/// Zustand der Weiterleitung.
#[derive(Clone)]
pub struct ProxyState {
    /// Registry der geladenen Plugins.
    pub registry: PluginRegistry,
    /// HTTP-Client für die Weiterleitung.
    pub client: Client,
}

/// Baut die Routen für die Pluginweiterleitung.
///
/// Die Routen sind ausdrücklich registriert und fangen keine fremden Pfade
/// auf. Der Server darf den Router deshalb einhängen, ohne dass ein
/// unbekannter Pfad still an ein Plugin gelangt.
pub fn router(registry: PluginRegistry) -> axum::Router {
    let zustand = ProxyState {
        registry,
        client: Client::new(),
    };
    axum::Router::new()
        .route("/api/plugins/{name}", any(weiterleitung_ohne_pfad))
        .route("/api/plugins/{name}/{*rest}", any(weiterleitung_mit_pfad))
        .with_state(zustand)
}

/// Leitet eine Anfrage ohne weiteren Pfad an das genannte Plugin weiter.
async fn weiterleitung_ohne_pfad(
    State(zustand): State<ProxyState>,
    Path(name): Path<String>,
    anfrage: Request,
) -> Response {
    antworte(zustand, &name, "", anfrage).await
}

/// Leitet eine Anfrage mit weiterem Pfad an das genannte Plugin weiter.
async fn weiterleitung_mit_pfad(
    State(zustand): State<ProxyState>,
    Path((name, rest)): Path<(String, String)>,
    anfrage: Request,
) -> Response {
    antworte(zustand, &name, &rest, anfrage).await
}

/// Fehler der Weiterleitung.
///
/// Die Fehlerhülle des Servers kennt nur `ok` und `error`. Dieser Typ hält
/// zusätzlich den HTTP-Statuscode, weil ein zu großer Rumpf vom übrigen
/// Netzwerkfehler unterschieden werden muss.
enum ProxyFehler {
    /// Fachlicher Fehler aus dem Kern oder aus der Registry.
    App(AppError),
    /// Der Rumpf überschreitet die Grenze.
    ZuGross,
}

/// Führt die Weiterleitung aus und bildet Fehler in die Fehlerhülle ab.
async fn antworte(zustand: ProxyState, name: &str, rest: &str, anfrage: Request) -> Response {
    match leite_weiter(&zustand, name, rest, anfrage).await {
        Ok(antwort) => antwort,
        Err(fehler) => fehlerantwort(fehler),
    }
}

/// Bildet einen Fehler in die Fehlerhülle des Servers ab.
fn fehlerantwort(fehler: ProxyFehler) -> Response {
    let (code, meldung) = match fehler {
        ProxyFehler::App(AppError::Conflict(_)) => (StatusCode::NOT_FOUND, String::new()),
        ProxyFehler::App(AppError::Validation(_)) => (StatusCode::BAD_REQUEST, String::new()),
        ProxyFehler::App(fehler) => (StatusCode::BAD_GATEWAY, fehler.to_string()),
        ProxyFehler::ZuGross => (
            StatusCode::PAYLOAD_TOO_LARGE,
            format!("Anfrage zu groß, höchstens {MAX_ROHKOERPER} Bytes sind erlaubt"),
        ),
    };
    let meldung = match meldung.is_empty() {
        true => "Pluginanfrage fehlgeschlagen".to_string(),
        false => meldung,
    };
    let text = serde_json::json!({ "ok": false, "error": meldung });
    Response::builder()
        .status(code)
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&text).unwrap_or_default()))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

/// Führt die Weiterleitung aus.
async fn leite_weiter(
    zustand: &ProxyState,
    name: &str,
    rest: &str,
    anfrage: Request,
) -> Result<Response, ProxyFehler> {
    let handle = zustand
        .registry
        .lookup_http(name)
        .await
        .map_err(ProxyFehler::App)?;
    let info = handle.info().await;
    let basis = info.http_base_url.ok_or_else(|| {
        ProxyFehler::App(AppError::Conflict(format!(
            "Plugin {name} bietet keine HTTP-Schnittstelle an"
        )))
    })?;
    let basis = Url::parse(&basis).map_err(|fehler| {
        ProxyFehler::App(AppError::Validation(format!(
            "Pluginadresse unbrauchbar: {fehler}"
        )))
    })?;
    pruefe_lokale_adresse(&basis).map_err(ProxyFehler::App)?;

    let ziel = baue_ziel(&basis, rest).map_err(ProxyFehler::App)?;

    let (teile, koerperteil) = anfrage.into_parts();
    let koerper = sammle_koerper(koerperteil).await?;

    let mut anfrage_aus = zustand
        .client
        .request(teile.method.clone(), ziel.as_str())
        .body(koerper.to_vec());

    for (kopf, wert) in teile.headers.iter() {
        if ist_gesperrt(kopf.as_str()) {
            continue;
        }
        anfrage_aus = anfrage_aus.header(kopf.clone(), wert.clone());
    }

    let antwort = anfrage_aus.send().await.map_err(|fehler| {
        ProxyFehler::App(AppError::Http(format!("Plugin antwortete nicht: {fehler}")))
    })?;

    let status = antwort.status();
    let headers = antwort.headers().clone();
    let bytes = antwort.bytes().await.map_err(|fehler| {
        ProxyFehler::App(AppError::Http(format!("Antwort unlesbar: {fehler}")))
    })?;

    let mut baue = Response::builder().status(status);
    for (kopf, wert) in headers.iter() {
        if ist_gesperrt(kopf.as_str()) {
            continue;
        }
        baue = baue.header(kopf.clone(), wert.clone());
    }
    Ok(baue
        .body(Body::from(bytes.to_vec()))
        .unwrap_or_else(|_| StatusCode::BAD_GATEWAY.into_response()))
}

/// Liest den Rumpf ein und überschreitet die Grenze nicht.
///
/// Die Prüfung passiert während des Lesens. Ein Rumpf, der die Grenze
/// überschreitet, bricht ab, sobald die Grenze erreicht ist. So belegt eine
/// einzelne Anfrage nicht mehr Speicher als erlaubt.
async fn sammle_koerper(koerper: Body) -> Result<Vec<u8>, ProxyFehler> {
    let mut strom = koerper.into_data_stream();
    let mut gesammelt: Vec<u8> = Vec::new();
    while let Some(teil) = strom.next().await {
        let teil = teil.map_err(|fehler| {
            ProxyFehler::App(AppError::Http(format!("Anfrage unlesbar: {fehler}")))
        })?;
        if gesammelt.len() + teil.len() > MAX_ROHKOERPER {
            return Err(ProxyFehler::ZuGross);
        }
        gesammelt.extend_from_slice(&teil);
    }
    Ok(gesammelt)
}

/// Prüft, dass die gemeldete Adresse auf dem lokalen Rechner liegt.
///
/// Der Host vergleicht den geparsten Hostnamen. Ein Zeilenpräfix wäre unsicher,
/// weil `http://127.0.0.1.example.com` mit einem Präfix beginnt und `http://localhost`
/// auch `http://localhost.evil` einschließen würde.
fn pruefe_lokale_adresse(adresse: &Url) -> AppResult<()> {
    if adresse.scheme() != "http" {
        return Err(AppError::Validation(format!(
            "Pluginadresse {} nutzt nicht das Schema http",
            adresse.scheme()
        )));
    }
    if !adresse.username().is_empty() || adresse.password().is_some() {
        return Err(AppError::Validation(
            "Pluginadresse darf keine Zugangsdaten enthalten".to_string(),
        ));
    }
    let host = adresse.host_str().unwrap_or_default();
    let erlaubt = match adresse.host() {
        Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
        Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
        Some(url::Host::Domain(domain)) => domain.eq_ignore_ascii_case("localhost"),
        None => false,
    };
    if !erlaubt {
        return Err(AppError::Validation(format!(
            "Pluginadresse {host} liegt nicht auf 127.0.0.1"
        )));
    }
    Ok(())
}

/// Baut die Zieladresse aus der Pluginbasis und dem angefragten Restpfad.
///
/// Pfadabschnitte wie `..` werden abgewiesen. Sie würden den Pfad der
/// Pluginbasis verlassen und könnten auf dem Pluginrechner beliebige Pfade
/// erreichen.
fn baue_ziel(basis: &Url, rest: &str) -> AppResult<Url> {
    let mut ziel = basis.clone();
    let basis_pfad = basis.path().trim_end_matches('/');
    let rest = rest.trim_start_matches('/');
    if rest.is_empty() {
        ziel.set_path(if basis_pfad.is_empty() {
            "/"
        } else {
            basis_pfad
        });
        return Ok(ziel);
    }
    for abschnitt in rest.split('/') {
        if abschnitt == ".." || abschnitt == "." {
            return Err(AppError::Validation(
                "Pfadabschnitte .. und . sind in Pluginpfaden nicht erlaubt".to_string(),
            ));
        }
    }
    ziel.set_path(&format!("{basis_pfad}/{rest}"));
    Ok(ziel)
}

/// Prüft, ob eine Kopfzeile nicht weitergeleitet werden darf.
fn ist_gesperrt(kopf: &str) -> bool {
    GESPERRTE_HEADER
        .iter()
        .any(|verboten| kopf.eq_ignore_ascii_case(verboten))
}

/// Wandelt eine Headerliste in eine Axum-Headerliste.
///
/// Header, die der Host selbst setzt, werden nicht übernommen.
pub fn kopiere_header(quelle: &HeaderMap) -> HeaderMap {
    let mut ziel = HeaderMap::new();
    for (name, wert) in quelle.iter() {
        if ist_gesperrt(name.as_str()) {
            continue;
        }
        if let Ok(name) = HeaderName::from_bytes(name.as_str().as_bytes()) {
            if let Ok(wert) = HeaderValue::from_bytes(wert.as_bytes()) {
                ziel.insert(name, wert);
            }
        }
    }
    ziel
}

/// Erzeugt die Standardanfrage für Tests.
pub fn testanfrage(uri: &str, koerper: Vec<u8>) -> Request {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(koerper))
        .unwrap_or_else(|_| Request::new(Body::empty()))
}

/// Bequemer Zugriff auf die Registry aus dem Zustand.
pub fn registry_aus(state: &ProxyState) -> Arc<PluginRegistry> {
    Arc::new(state.registry.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn adresse(text: &str) -> Url {
        Url::parse(text).expect("Testadresse ist lesbar")
    }

    #[test]
    fn fremde_adressen_werden_abgewiesen() {
        let fehler = pruefe_lokale_adresse(&adresse("http://10.0.0.5:8080")).unwrap_err();
        assert!(matches!(fehler, AppError::Validation(_)));
    }

    #[test]
    fn praefix_trick_wird_abgewiesen() {
        let fehler = pruefe_lokale_adresse(&adresse("http://127.0.0.1.example.com")).unwrap_err();
        assert!(matches!(fehler, AppError::Validation(_)));
        let fehler = pruefe_lokale_adresse(&adresse("http://localhost.evil")).unwrap_err();
        assert!(matches!(fehler, AppError::Validation(_)));
    }

    #[test]
    fn lokale_adressen_werden_angenommen() {
        assert!(pruefe_lokale_adresse(&adresse("http://127.0.0.1:9000")).is_ok());
        assert!(pruefe_lokale_adresse(&adresse("http://localhost:9000")).is_ok());
        assert!(pruefe_lokale_adresse(&adresse("http://[::1]:9000")).is_ok());
    }

    #[test]
    fn https_und_zugangsdaten_werden_abgewiesen() {
        let fehler = pruefe_lokale_adresse(&adresse("https://127.0.0.1:9000")).unwrap_err();
        assert!(matches!(fehler, AppError::Validation(_)));
        let fehler = pruefe_lokale_adresse(&adresse("http://user:pw@127.0.0.1:9000")).unwrap_err();
        assert!(matches!(fehler, AppError::Validation(_)));
    }

    #[test]
    fn zielpfad_bleibt_in_der_pluginbasis() {
        let ziel = baue_ziel(&adresse("http://127.0.0.1:9000/base"), "a/b").unwrap();
        assert_eq!(ziel.as_str(), "http://127.0.0.1:9000/base/a/b");

        let ziel = baue_ziel(&adresse("http://127.0.0.1:9000"), "").unwrap();
        assert_eq!(ziel.as_str(), "http://127.0.0.1:9000/");
    }

    #[test]
    fn pfad_ausbruch_wird_abgewiesen() {
        let fehler = baue_ziel(&adresse("http://127.0.0.1:9000/base"), "../admin").unwrap_err();
        assert!(matches!(fehler, AppError::Validation(_)));
    }

    #[test]
    fn zugangsdaten_des_hosts_werden_nicht_weitergeleitet() {
        let mut quelle = HeaderMap::new();
        quelle.insert("x-admin-token", HeaderValue::from_static("geheim"));
        quelle.insert("authorization", HeaderValue::from_static("Bearer geheim"));
        quelle.insert("cookie", HeaderValue::from_static("a=b"));
        quelle.insert("content-type", HeaderValue::from_static("application/json"));

        let ziel = kopiere_header(&quelle);
        assert!(!ziel.contains_key("x-admin-token"));
        assert!(!ziel.contains_key("authorization"));
        assert!(!ziel.contains_key("cookie"));
        assert!(ziel.contains_key("content-type"));
    }
}
