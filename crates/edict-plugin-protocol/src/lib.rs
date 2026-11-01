//! Protokoll zwischen Edict-Host und Hotplug-Plugins.
//!
//! Ein Plugin ist ein eigener Prozess. Der Host startet ihn, spricht mit ihm
//! über JSON-RPC 2.0 auf stdio und beendet ihn wieder. Die Nachrichten sind
//! mit einem `Content-Length`-Kopf gerahmt, wie es die Sprachserver-
//! Konvention vorsieht. Damit bleibt die Leitung für Mehrbyte-Zeichen
//! eindeutig, auch wenn eine Nutzlast Zeilenumbrüche enthält.
//!
//! Das Protokoll kennt keine Geheimnisse. Ein Plugin erhält weder das
//! Admin-Token noch Zugangsdaten für die Datenbank. Es bekommt nur die
//! Aufträge, die der Host ihm schreibt.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt};

/// Fehler des Protokollrahmens.
#[derive(Debug, thiserror::Error)]
pub enum ProtocolError {
    #[error("Verbindung zum Plugin wurde geschlossen")]
    Closed,

    #[error("Ungültige Nachricht: {0}")]
    Invalid(String),

    #[error("Nachricht ist zu groß: {0} Bytes")]
    TooLarge(usize),

    #[error("Fehler beim Lesen der Nachricht: {0}")]
    Io(#[from] std::io::Error),

    #[error("Fehler beim Serialisieren der Nachricht: {0}")]
    Json(#[from] serde_json::Error),
}

/// Maximale Länge einer Nutzlast in Bytes.
pub const MAX_PAYLOAD_BYTES: usize = 8 * 1024 * 1024;

/// JSON-RPC-Nachricht im Rahmen.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Version des Protokolls. Immer `2.0`.
    pub jsonrpc: String,
    /// Kennung einer Anfrage. Antworten tragen dieselbe Kennung.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    /// Name der Methode bei Anfragen und Hinweisen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    /// Nutzlast der Anfrage oder der Antwort.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
    /// Ergebnis einer Antwort.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    /// Fehler einer Antwort.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

/// Fehlerobjekt einer JSON-RPC-Antwort.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcError {
    /// Zahlencode nach JSON-RPC.
    pub code: i32,
    /// Kurzbeschreibung des Fehlers.
    pub message: String,
    /// Optionale Zusatzinformation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl Message {
    /// Baut eine Anfrage.
    pub fn request(id: Value, method: impl Into<String>, params: Value) -> Self {
        Message {
            jsonrpc: "2.0".to_string(),
            id: Some(id),
            method: Some(method.into()),
            params: Some(params),
            result: None,
            error: None,
        }
    }

    /// Baut eine Antwort auf eine Anfrage.
    pub fn response(id: Value, result: Value) -> Self {
        Message {
            jsonrpc: "2.0".to_string(),
            id: Some(id),
            method: None,
            params: None,
            result: Some(result),
            error: None,
        }
    }

    /// Baut eine Fehlerantwort auf eine Anfrage.
    pub fn error_response(id: Value, code: i32, message: impl Into<String>) -> Self {
        Message {
            jsonrpc: "2.0".to_string(),
            id: Some(id),
            method: None,
            params: None,
            result: None,
            error: Some(RpcError {
                code,
                message: message.into(),
                data: None,
            }),
        }
    }

    /// Baut einen Hinweis ohne Antwort.
    pub fn notification(method: impl Into<String>, params: Value) -> Self {
        Message {
            jsonrpc: "2.0".to_string(),
            id: None,
            method: Some(method.into()),
            params: Some(params),
            result: None,
            error: None,
        }
    }

    /// Meldet, ob die Nachricht eine Anfrage mit erwarteter Antwort ist.
    pub fn is_request(&self) -> bool {
        self.method.is_some() && self.id.is_some()
    }
}

/// Version des Pluginprotokolls.
///
/// Der Host nimmt nur Plugins an, deren Manifest undwhose Antwort auf
/// `initialize` dieselbe Version nennen. Eine Abweichung beendet den
/// Prozess mit dem Fehlercode [`RPC_INVALID_PARAMS`].
pub const PROTOCOL_VERSION: u32 = 1;

/// Fehlercode nach JSON-RPC für eine unbekannte Methode.
pub const RPC_METHOD_NOT_FOUND: i32 = -32601;

/// Fehlercode nach JSON-RPC für ungültige Parameter oder Protokollversion.
pub const RPC_INVALID_PARAMS: i32 = -32602;

/// Fehlercode für einen Fehler im Plugin selbst.
pub const RPC_PLUGIN_ERROR: i32 = -32000;

/// Fehlercode für eine überlastete Warteschlange.
pub const RPC_OVERLOADED: i32 = -32001;

/// Methoden zwischen Host und Plugin.
pub mod method {
    /// Der Host meldet sich an und übergibt die Rahmenbedingungen.
    ///
    /// Die Antwort nennt `protocolVersion`, Namen, Version und Fähigkeiten
    /// des Plugins.
    pub const INITIALIZE: &str = "initialize";
    /// Der Host fragt den Zustand des Plugins ab.
    ///
    /// Die Antwort nennt `ready`, `degraded` oder `failed`, die Zeit seit dem
    /// Start, den letzten Fehler und die Länge der Warteschlange.
    pub const HEALTH: &str = "health";
    /// Der Host schickt dem Plugin ein Ereignis. Keine Antwort erwartet.
    pub const EVENT: &str = "event";
    /// Ruft eine benannte Fähigkeit auf. In beide Richtungen möglich.
    pub const INVOKE: &str = "invoke";
    /// Das Plugin meldet ein Domainereignis. Der Host vergibt Kennung und
    /// Zeitstempel und legt Ereignis und Outbox an.
    pub const EMIT_EVENT: &str = "emit_event";
    /// Der Host fordert das Plugin zum geordneten Beenden auf.
    pub const SHUTDOWN: &str = "shutdown";
    /// Das Plugin schreibt einen Protokolleintrag. Keine Antwort erwartet.
    pub const LOG: &str = "log";
}

/// Zustandsmeldung eines Plugins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginState {
    /// Das Plugin arbeitet normal.
    Ready,
    /// Das Plugin arbeitet eingeschränkt, etwa bei voller Warteschlange.
    Degraded,
    /// Das Plugin ist nicht arbeitsfähig.
    Failed,
}

/// Antwort des Plugins auf `initialize`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeResult {
    /// Protokollversion des Plugins.
    pub protocol_version: u32,
    /// Eindeutiger Name des Plugins.
    pub name: String,
    /// Version des Plugins nach SemVer.
    pub version: String,
    /// Fähigkeiten, die der Host bei diesem Plugin aufrufen darf.
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// Ereignisarten, die das Plugin empfangen möchte.
    #[serde(default)]
    pub subscriptions: Vec<String>,
    /// Adresse, auf der das Plugin HTTP-Routen anbietet. Nur `127.0.0.1`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http_base_url: Option<String>,
}

/// Antwort des Plugins auf `health`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResult {
    /// Zustand des Plugins.
    pub state: PluginState,
    /// Sekunden seit dem Start des Prozesses.
    pub uptime_seconds: u64,
    /// Anzahl der Ereignisse in der Warteschlange.
    pub queued_events: u64,
    /// Anzahl der vom Host verworfenen Ereignisse wegen voller Warteschlange.
    #[serde(default)]
    pub dropped_events: u64,
    /// Text des letzten Fehlers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

/// Antwort des Plugins auf `invoke`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvokeResult {
    /// Ergebnis der Fähigkeit.
    pub value: Value,
}

/// Liest eine Nachricht aus einem zeilengenauen Leser.
///
/// Der Leser liefert zuerst die Kopfzeilen, danach die Nutzlast in der
/// angegebenen Länge.
pub async fn read_message<R>(reader: &mut R) -> Result<Message, ProtocolError>
where
    R: AsyncBufRead + Unpin,
{
    let mut length: Option<usize> = None;
    loop {
        let mut line = String::new();
        let read = reader.read_line(&mut line).await?;
        if read == 0 {
            return Err(ProtocolError::Closed);
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }
        let (name, value) = match trimmed.split_once(':') {
            Some(parts) => parts,
            None => {
                return Err(ProtocolError::Invalid(format!(
                    "Kopfzeile ohne Doppelpunkt: {trimmed}"
                )))
            }
        };
        if name.trim().eq_ignore_ascii_case("Content-Length") {
            length = Some(value.trim().parse::<usize>().map_err(|_| {
                ProtocolError::Invalid(format!("Ungültige Content-Length: {}", value.trim()))
            })?);
        }
    }

    let length = length
        .ok_or_else(|| ProtocolError::Invalid("Kopfzeile Content-Length fehlt".to_string()))?;
    if length > MAX_PAYLOAD_BYTES {
        return Err(ProtocolError::TooLarge(length));
    }

    let mut buffer = vec![0u8; length];
    tokio::io::AsyncReadExt::read_exact(reader, &mut buffer).await?;
    let text = String::from_utf8(buffer)
        .map_err(|error| ProtocolError::Invalid(format!("Nutzlast ist kein UTF-8: {error}")))?;
    let message = serde_json::from_str(&text)?;
    Ok(message)
}

/// Schreibt eine Nachricht mit `Content-Length`-Kopf.
pub async fn write_message<W>(writer: &mut W, message: &Message) -> Result<(), ProtocolError>
where
    W: AsyncWrite + Unpin,
{
    let text = serde_json::to_string(message)?;
    let bytes = text.as_bytes();
    if bytes.len() > MAX_PAYLOAD_BYTES {
        return Err(ProtocolError::TooLarge(bytes.len()));
    }
    writer
        .write_all(format!("Content-Length: {}\r\n\r\n", bytes.len()).as_bytes())
        .await?;
    writer.write_all(bytes).await?;
    writer.flush().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn nachricht_ueberlebt_zeilenumbrueche_in_der_nutzlast() {
        let mut buffer: Vec<u8> = Vec::new();
        let message = Message::request(
            Value::from(1),
            method::EVENT,
            serde_json::json!({"text": "erste Zeile\nzweite Zeile"}),
        );
        write_message(&mut buffer, &message).await.unwrap();

        let mut cursor = std::io::Cursor::new(buffer);
        let mut lines = tokio::io::BufReader::new(&mut cursor);
        let gelesen = read_message(&mut lines).await.unwrap();
        assert_eq!(gelesen.method.as_deref(), Some(method::EVENT));
        assert_eq!(
            gelesen.params.unwrap()["text"],
            Value::from("erste Zeile\nzweite Zeile")
        );
    }

    #[tokio::test]
    async fn fehlender_kopf_wird_abgewiesen() {
        let mut cursor = std::io::Cursor::new(b"{}\r\n".to_vec());
        let mut lines = tokio::io::BufReader::new(&mut cursor);
        let fehler = read_message(&mut lines).await.unwrap_err();
        assert!(matches!(fehler, ProtocolError::Invalid(_)));
    }
}
