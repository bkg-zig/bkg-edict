//! HTTP-Eingang von gitclaw.
//!
//! Das Plugin nimmt Webhooks auf `127.0.0.1` an. Der Host leitet die
//! Anfragen der Clients an diese Adresse weiter. Andere Adressen sind nicht
//! möglich, weil der Listener fest an das Loopback-Netz gebunden wird.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Json;
use edict_plugin_protocol::method;
use serde_json::{json, Value};

use crate::review::{auftrag_nutzlast, werte_aus, WebhookArt};
use crate::rpc::RpcPeer;
use crate::signature::SignaturPruefer;

/// Kopfzeile mit der Signatur des Webhooks.
pub const SIGNATUR_KOPFZEILE: &str = "x-hub-signature-256";

/// Feste Obergrenze für einen Webhook-Körper.
pub const MAX_KOERPER: usize = 4 * 1024 * 1024;

/// Frist für einen Auftrag an den Host.
const AUFTRAG_FRIST: std::time::Duration = std::time::Duration::from_secs(30);

/// Zustand des HTTP-Eingangs.
pub struct WebhookZustand {
    /// Verbindung zum Host für Aufträge und Meldungen.
    pub verbindung: Arc<RpcPeer>,
    /// Prüfer der Signatur.
    pub pruefer: Arc<dyn SignaturPruefer>,
}

/// Baut den Router des Webhook-Eingangs.
pub fn router(zustand: Arc<WebhookZustand>) -> axum::Router {
    axum::Router::new()
        .route("/health", get(health))
        .route("/webhook/{art}", post(webhook))
        .with_state(zustand)
}

/// Meldet den Zustand des Plugins.
async fn health() -> Response {
    (StatusCode::OK, Json(json!({"state": "ready"}))).into_response()
}

/// Nimmt einen Webhook an.
///
/// Der Ablauf ist: Signatur prüfen, Ereignis auswerten, Prüfauftrag an den
/// Host senden, Ergebnis an den Host melden.
async fn webhook(
    State(zustand): State<Arc<WebhookZustand>>,
    Path(art): Path<String>,
    kopfzeilen: HeaderMap,
    koerper: Bytes,
) -> Response {
    if koerper.len() > MAX_KOERPER {
        return (StatusCode::PAYLOAD_TOO_LARGE, "Körper zu groß").into_response();
    }
    if !zustand.pruefer.bereit() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "Kein Signaturschlüssel hinterlegt",
        )
            .into_response();
    }

    let signatur = kopfzeilen
        .get(SIGNATUR_KOPFZEILE)
        .and_then(|wert| wert.to_str().ok())
        .unwrap_or_default();
    if !zustand.pruefer.pruefe(&koerper, signatur) {
        return (StatusCode::UNAUTHORIZED, "Signatur ungültig").into_response();
    }

    let art = WebhookArt::aus_name(&art);
    if !art.bekannt() {
        return (StatusCode::NOT_FOUND, "Ereignisart unbekannt").into_response();
    }
    let Some(ereignis) = werte_aus(art, &koerper) else {
        return (StatusCode::BAD_REQUEST, "Körper ist kein gültiges JSON").into_response();
    };

    let auftrag = auftrag_nutzlast(&ereignis);
    let ergebnis = match zustand
        .verbindung
        .ruf(
            method::INVOKE,
            json!({
                "capability": "review.request",
                "params": auftrag,
            }),
            AUFTRAG_FRIST,
        )
        .await
    {
        Ok(ergebnis) => ergebnis,
        Err(fehler) => {
            tracing::warn!("Prüfauftrag nicht angenommen: {}", fehler.message);
            return (StatusCode::BAD_GATEWAY, "Host nicht erreichbar").into_response();
        }
    };

    let meldung = json!({
        "topic": "task.review.result",
        "event_type": "task.review.result",
        "producer": "gitclaw",
        "payload": {
            "art": ereignis.art.name(),
            "lager": ereignis.lager,
            "dateien": ereignis.dateien,
            "zusammenfassung": ereignis.zusammenfassung,
            "ergebnis": ergebnis.get("value").cloned().unwrap_or(Value::Null),
        },
    });
    if let Err(fehler) = zustand
        .verbindung
        .ruf(method::EMIT_EVENT, meldung, AUFTRAG_FRIST)
        .await
    {
        tracing::warn!("Ergebnis konnte nicht gemeldet werden: {}", fehler.message);
        return (
            StatusCode::BAD_GATEWAY,
            "Ergebnis konnte nicht gemeldet werden",
        )
            .into_response();
    }

    (StatusCode::ACCEPTED, "Prüfauftrag angenommen").into_response()
}

/// Bindet den Listener an `127.0.0.1`.
///
/// Der Port 0 lässt das Betriebssystem einen freien Port wählen. Die
/// tatsächliche Adresse wird dem Host beim Handshake gemeldet.
pub async fn binde(port: u16) -> std::io::Result<(SocketAddr, tokio::net::TcpListener)> {
    let adresse = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(adresse).await?;
    let adresse = listener.local_addr()?;
    Ok((adresse, listener))
}

/// Startet den HTTP-Eingang.
pub async fn starte(
    listener: tokio::net::TcpListener,
    zustand: Arc<WebhookZustand>,
) -> std::io::Result<()> {
    axum::serve(listener, router(zustand)).await
}
