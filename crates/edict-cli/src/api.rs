//! Zugriff auf die HTTP-Schnittstelle des Edict-Servers.

use std::fmt;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use reqwest::{Client, Url};
use serde_json::Value;

/// Wartezeit für Anfragen an den Server.
const TIMEOUT_SEC: u64 = 30;

/// Kopfzeile für das Admin-Token neben der Bearer-Variante.
const ADMIN_HEADER: &str = "x-admin-token";

/// Fehler einer Serverantwort.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiError {
    /// Der Server hat einen Fehlerstatus geliefert.
    Status { status: u16, message: String },
    /// Die Antwort war kein gültiges JSON.
    InvalidJson { status: u16, message: String },
    /// Die Antwort meldet `ok: false`.
    Rejected { status: u16, message: String },
}

impl fmt::Display for ApiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApiError::Status { status, message } => {
                write!(
                    formatter,
                    "Der Server antwortete mit HTTP {status}: {message}"
                )
            }
            ApiError::InvalidJson { status, message } => write!(
                formatter,
                "Die Antwort des Servers (HTTP {status}) ist kein gültiges JSON: {message}"
            ),
            ApiError::Rejected { status, message } => write!(
                formatter,
                "Der Server hat die Anfrage abgelehnt (HTTP {status}): {message}"
            ),
        }
    }
}

impl std::error::Error for ApiError {}

/// Client für die Server-API.
pub struct ApiClient {
    base_url: Url,
    token: Option<String>,
    client: Client,
}

impl ApiClient {
    /// Erstellt einen Client für die angegebene Server-Adresse.
    pub fn new(base_url: &str, token: Option<String>) -> Result<Self> {
        let base = parse_base_url(base_url)?;
        let client = Client::builder()
            .timeout(Duration::from_secs(TIMEOUT_SEC))
            .build()
            .context("Der HTTP-Client konnte nicht gestartet werden.")?;
        Ok(Self {
            base_url: base,
            token,
            client,
        })
    }

    /// Basisadresse des Servers.
    pub fn base_url(&self) -> &Url {
        &self.base_url
    }

    /// Liest eine Route ein.
    pub async fn get(&self, route: &str) -> Result<Value> {
        let url = endpoint(&self.base_url, route)?;
        let response = self
            .client
            .get(url.clone())
            .headers(self.headers())
            .send()
            .await
            .map_err(|error| transport_error(&url, error))?;
        self.finish(response).await
    }

    /// Schreibt eine Route.
    pub async fn post(&self, route: &str, body: &Value) -> Result<Value> {
        let url = endpoint(&self.base_url, route)?;
        let response = self
            .client
            .post(url.clone())
            .headers(self.headers())
            .json(body)
            .send()
            .await
            .map_err(|error| transport_error(&url, error))?;
        self.finish(response).await
    }

    /// Liest Statuscode und Text der Antwort aus.
    async fn finish(&self, response: reqwest::Response) -> Result<Value> {
        let status = response.status().as_u16();
        let body = response
            .text()
            .await
            .context("Die Antwort des Servers konnte nicht gelesen werden.")?;
        evaluate_response(status, &body).map_err(|error| anyhow!(error))
    }

    /// Kopfbereiche der Anfrage.
    ///
    /// Das Admin-Token wird als Bearer-Token und zusätzlich als
    /// `X-Admin-Token` gesendet. Der Server akzeptiert beide Formen.
    fn headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        if let Some(token) = &self.token {
            if let Ok(value) = HeaderValue::from_str(&format!("Bearer {token}")) {
                headers.insert(AUTHORIZATION, value);
            }
            if let Ok(value) = HeaderValue::from_str(token) {
                headers.insert(ADMIN_HEADER, value);
            }
        }
        headers
    }
}

/// Wandelt einen Transportfehler in eine deutsche Meldung.
fn transport_error(url: &Url, error: reqwest::Error) -> anyhow::Error {
    if error.is_timeout() {
        anyhow!("Die Anfrage an {url} hat das Zeitlimit von {TIMEOUT_SEC} Sekunden überschritten.")
    } else {
        anyhow!("Der Server unter {url} ist nicht erreichbar: {error}")
    }
}

/// Prüft eine Basisadresse und entfernt Query und Fragment.
pub fn parse_base_url(value: &str) -> Result<Url> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("Die Server-Adresse ist leer."));
    }
    let mut url = Url::parse(trimmed)
        .map_err(|error| anyhow!("Die Server-Adresse „{trimmed}“ ist ungültig: {error}"))?;
    if url.scheme() != "http" && url.scheme() != "https" {
        return Err(anyhow!(
            "Die Server-Adresse muss mit http:// oder https:// beginnen: {trimmed}"
        ));
    }
    url.set_query(None);
    url.set_fragment(None);
    Ok(url)
}

/// Baut die vollständige Adresse einer Route.
///
/// Der Pfad der Basisadresse bleibt erhalten. So funktioniert ein Server
/// hinter einer Umleitung wie `https://host/edict/`.
pub fn endpoint(base: &Url, route: &str) -> Result<Url> {
    let mut url = base.clone();
    {
        let mut segments = url
            .path_segments_mut()
            .map_err(|_| anyhow!("Die Server-Adresse darf weder Query noch Fragment enthalten."))?;
        segments.pop_if_empty();
        for segment in route.split('/').filter(|part| !part.is_empty()) {
            segments.push(segment);
        }
    }
    Ok(url)
}

/// Wertet Statuscode und Inhalt einer Serverantwort aus.
///
/// Die schreibenden Routen antworten mit `{"ok": true, "message": "..."}` oder
/// `{"ok": false, "error": "..."}`. Fehlerstatuscodes werden immer als Fehler
/// gewertet, auch wenn der Inhalt kein `ok`-Feld trägt.
pub fn evaluate_response(status: u16, body: &str) -> Result<Value, ApiError> {
    let text = body.trim();
    let parsed: Option<Value> = serde_json::from_str(text).ok();
    match parsed {
        Some(Value::Object(map)) => {
            let message = map
                .get("error")
                .and_then(Value::as_str)
                .or_else(|| map.get("message").and_then(Value::as_str))
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| fallback_message(text));
            if status >= 400 {
                return Err(ApiError::Status { status, message });
            }
            if map.get("ok").and_then(Value::as_bool) == Some(false) {
                return Err(ApiError::Rejected { status, message });
            }
            Ok(Value::Object(map))
        }
        Some(value) => {
            if status >= 400 {
                Err(ApiError::Status {
                    status,
                    message: fallback_message(text),
                })
            } else {
                Ok(value)
            }
        }
        None => {
            if status >= 400 {
                Err(ApiError::Status {
                    status,
                    message: fallback_message(text),
                })
            } else {
                Err(ApiError::InvalidJson {
                    status,
                    message: fallback_message(text),
                })
            }
        }
    }
}

/// Ersatztext, wenn die Antwort keinen lesbaren Fehlertext trägt.
fn fallback_message(text: &str) -> String {
    if text.is_empty() {
        "keine weitere Angabe".to_string()
    } else {
        text.to_string()
    }
}

/// Liest den Erfolgs- oder Fehlertext aus einer Antwort.
pub fn message_of(value: &Value, fallback: &str) -> String {
    value
        .get("message")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .unwrap_or(fallback)
        .to_string()
}

/// Kleiner HTTP-Server für die Prüfung des Clients.
#[cfg(test)]
pub(crate) mod test_server {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    /// Nimmt genau eine Anfrage an und liefert eine feste Antwort.
    pub struct TestServer {
        pub(crate) url: String,
        pub(crate) handle: Option<std::thread::JoinHandle<String>>,
    }

    impl TestServer {
        /// Startet einen Server, der eine feste Antwort liefert.
        pub fn start(status: u16, body: &'static str) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let handle = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                let mut buffer = Vec::new();
                let mut chunk = [0u8; 1024];
                let head_end = loop {
                    let read = stream.read(&mut chunk).unwrap();
                    if read == 0 {
                        break buffer.len();
                    }
                    buffer.extend_from_slice(&chunk[..read]);
                    if let Some(position) =
                        buffer.windows(4).position(|window| window == b"\r\n\r\n")
                    {
                        break position + 4;
                    }
                };
                let head = String::from_utf8_lossy(&buffer[..head_end]).to_string();
                let length = head
                    .lines()
                    .find_map(|line| {
                        let mut parts = line.splitn(2, ':');
                        let name = parts.next().unwrap_or("").trim().to_lowercase();
                        let value = parts.next().unwrap_or("").trim().to_string();
                        (name == "content-length").then(|| value.parse::<usize>().ok())?
                    })
                    .unwrap_or(0);
                while buffer.len() < head_end + length {
                    let read = stream.read(&mut chunk).unwrap();
                    if read == 0 {
                        break;
                    }
                    buffer.extend_from_slice(&chunk[..read]);
                }
                let response = format!(
                    "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
                String::from_utf8_lossy(&buffer).to_string()
            });
            Self {
                url: format!("http://{address}"),
                handle: Some(handle),
            }
        }

        /// Wartet auf das Ende der Anfrage und liefert sie als Text.
        pub fn request(&mut self) -> String {
            self.handle.take().unwrap().join().unwrap()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_server::TestServer;
    use super::*;

    #[test]
    fn basisadresse_wird_geprueft() {
        assert!(parse_base_url("http://127.0.0.1:8080").is_ok());
        assert!(parse_base_url("https://edict.example.org").is_ok());
        assert!(parse_base_url("ftp://edict.example.org").is_err());
        assert!(parse_base_url("").is_err());
        assert!(parse_base_url("nicht-url").is_err());
    }

    #[test]
    fn route_wird_an_die_basisadresse_gehängt() {
        let base = parse_base_url("http://127.0.0.1:8080").unwrap();
        let url = endpoint(&base, "/api/live-status").unwrap();
        assert_eq!(url.as_str(), "http://127.0.0.1:8080/api/live-status");
    }

    #[test]
    fn pfad_der_basisadresse_bleibt_erhalten() {
        let base = parse_base_url("https://host/edict/").unwrap();
        let url = endpoint(&base, "/api/task-activity/JJC-1").unwrap();
        assert_eq!(url.as_str(), "https://host/edict/api/task-activity/JJC-1");
    }

    #[test]
    fn kennungen_werden_kodiert() {
        let base = parse_base_url("http://127.0.0.1:8080").unwrap();
        let url = endpoint(&base, "/api/task-activity/JJC 1").unwrap();
        assert_eq!(
            url.as_str(),
            "http://127.0.0.1:8080/api/task-activity/JJC%201"
        );
    }

    #[test]
    fn erfolgsantwort_wird_akzeptiert() {
        let value = evaluate_response(200, r#"{"ok":true,"message":"Aufgabe angelegt"}"#).unwrap();
        assert_eq!(value["ok"], true);
    }

    #[test]
    fn ablehnung_wird_als_fehler_gewertet() {
        let error = evaluate_response(200, r#"{"ok":false,"error":"title required"}"#).unwrap_err();
        assert_eq!(
            error,
            ApiError::Rejected {
                status: 200,
                message: "title required".to_string()
            }
        );
    }

    #[test]
    fn fehlerstatus_wird_als_fehler_gewertet() {
        let error =
            evaluate_response(400, r#"{"ok":false,"error":"taskId required"}"#).unwrap_err();
        assert_eq!(
            error,
            ApiError::Status {
                status: 400,
                message: "taskId required".to_string()
            }
        );
    }

    #[test]
    fn fehlerstatus_ohne_json_bleibt_lesbar() {
        let error = evaluate_response(500, "Serverfehler").unwrap_err();
        assert_eq!(
            error,
            ApiError::Status {
                status: 500,
                message: "Serverfehler".to_string()
            }
        );
    }

    #[test]
    fn ungueltiges_json_wird_gemeldet() {
        let error = evaluate_response(200, "kein json").unwrap_err();
        assert!(matches!(error, ApiError::InvalidJson { status: 200, .. }));
    }

    #[test]
    fn leere_antwort_erhaelt_platzhalter() {
        let error = evaluate_response(503, "").unwrap_err();
        assert_eq!(
            error,
            ApiError::Status {
                status: 503,
                message: "keine weitere Angabe".to_string()
            }
        );
    }

    #[test]
    fn client_liest_eine_route() {
        let mut server = TestServer::start(200, r#"{"tasks":[],"syncStatus":{"ok":true}}"#);
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let client = ApiClient::new(&server.url, None).unwrap();
        let value = runtime.block_on(client.get("/api/live-status")).unwrap();
        assert_eq!(value["tasks"].as_array().unwrap().len(), 0);
        let request = server.request();
        assert!(request.starts_with("GET /api/live-status HTTP/1.1"));
    }

    #[test]
    fn client_sendet_inhalt_und_token() {
        let mut server = TestServer::start(200, r#"{"ok":true,"message":"Aufgabe angelegt"}"#);
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let client = ApiClient::new(&server.url, Some("geheim".to_string())).unwrap();
        let value = runtime
            .block_on(client.post("/api/create-task", &serde_json::json!({"title":"Test"})))
            .unwrap();
        assert_eq!(value["ok"], true);
        let request = server.request();
        assert!(request.starts_with("POST /api/create-task HTTP/1.1"));
        assert!(request.contains("authorization: Bearer geheim"));
        assert!(request.contains("x-admin-token: geheim"));
        assert!(request.contains("{\"title\":\"Test\"}"));
    }

    #[test]
    fn client_meldet_fehlerstatus_des_servers() {
        let mut server = TestServer::start(400, r#"{"ok":false,"error":"title required"}"#);
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let client = ApiClient::new(&server.url, None).unwrap();
        let error = runtime
            .block_on(client.post("/api/create-task", &serde_json::json!({})))
            .unwrap_err();
        assert!(format!("{error:#}").contains("title required"));
        server.request();
    }
}
