//! HTTP- und WebSocket-Zugriff auf den Edict-Server.
//!
//! Alle Routen und Feldnamen entsprechen dem bestehenden Dashboard-Vertrag.
//! Fehlermeldungen werden auf Deutsch formuliert, damit die Oberflaeche
//! keine englischen Servertexte anzeigt.

use gloo_net::http::Request;
use serde::de::DeserializeOwned;
use serde_json::{json, Map, Value};

use crate::models::{
    ActionResult, AgentActivityData, AgentConfig, AgentsStatusData, ChangeLogEntry, LiveStatus,
    MorningBrief, NewTask, OfficialsData, SchedulerStateData, TaskActivityData, TodoItem,
};

/// Basis-URL der API. Ohne die Umgebungsvariable `EDICT_API_BASE` wird der
/// Host der Seite verwendet.
const API_BASE: &str = match option_env!("EDICT_API_BASE") {
    Some(value) => value,
    None => "",
};

/// Themen, die die Oberflaeche im WebSocket abonniert.
pub const SUBSCRIBED_TOPICS: [&str; 6] = [
    "task.created",
    "task.status",
    "task.completed",
    "task.closed",
    "agent.heartbeat",
    "agent.todo.update",
];

/// Fehler einer Serveranfrage mit deutscher Meldung.
#[derive(Debug, Clone, PartialEq)]
pub struct ApiError {
    message: String,
}

impl ApiError {
    /// Neue Fehlermeldung.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    /// Deutsche Fehlermeldung.
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ApiError {}

/// Baut die vollstaendige URL einer Route.
fn endpoint(path: &str) -> String {
    format!("{}{}", API_BASE.trim_end_matches('/'), path)
}

/// Schreibt eine technische Meldung in die Browserkonsole.
fn log_technical(message: &str) {
    web_sys::console::error_1(&wasm_bindgen::JsValue::from_str(message));
}

/// Liest eine JSON-Antwort einer Lese-Route.
async fn get_json<T: DeserializeOwned>(path: &str) -> Result<T, ApiError> {
    let response = Request::get(&endpoint(path)).send().await.map_err(|_| {
        ApiError::new(format!(
            "Der Server ist nicht erreichbar. Die Route {path} konnte nicht abgerufen werden."
        ))
    })?;
    let status = response.status();
    if !(200..300).contains(&status) {
        return Err(ApiError::new(format!(
            "Der Server antwortet auf {path} mit dem Status {status}."
        )));
    }
    response.json::<T>().await.map_err(|_| {
        ApiError::new(format!(
            "Die Antwort von {path} liess sich nicht als JSON lesen."
        ))
    })
}

/// Fuehrt eine schreibende Aktion aus und meldet das Ergebnis.
async fn post_action(path: &str, body: &Value) -> Result<ActionResult, ApiError> {
    let request = Request::post(&endpoint(path)).json(body).map_err(|error| {
        log_technical(&format!("{path} konnte nicht serialisiert werden: {error}"));
        ApiError::new(format!(
            "Die Aktion {path} konnte nicht vorbereitet werden."
        ))
    })?;
    let response = request.send().await.map_err(|_| {
        ApiError::new(format!(
            "Der Server ist nicht erreichbar. Die Aktion {path} konnte nicht gesendet werden."
        ))
    })?;
    let status = response.status();
    let result = response.json::<ActionResult>().await.map_err(|_| {
        ApiError::new(format!(
            "Die Antwort von {path} liess sich nicht als JSON lesen."
        ))
    })?;
    if !(200..300).contains(&status) || !result.ok {
        if let Some(reason) = &result.error {
            log_technical(&format!("{path} wurde abgelehnt: {reason}"));
        }
        return Err(ApiError::new(
            "Der Server hat die Aktion abgelehnt. Die Aufgabe wurde nicht veraendert.",
        ));
    }
    Ok(result)
}

/// percent-enkodiert ein Pfadsegment.
fn encode_segment(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(*byte as char)
            }
            other => encoded.push_str(&format!("%{other:02X}")),
        }
    }
    encoded
}

/// Live-Status der Aufgabenliste.
pub async fn live_status() -> Result<LiveStatus, ApiError> {
    get_json("/api/live-status").await
}

/// Status aller Agenten.
pub async fn agents_status() -> Result<AgentsStatusData, ApiError> {
    get_json("/api/agents-status").await
}

/// Aktivitaet eines Agenten.
pub async fn agent_activity(agent_id: &str) -> Result<AgentActivityData, ApiError> {
    get_json(&format!("/api/agent-activity/{}", encode_segment(agent_id))).await
}

/// Aktivitaet einer Aufgabe.
pub async fn task_activity(task_id: &str) -> Result<TaskActivityData, ApiError> {
    get_json(&format!("/api/task-activity/{}", encode_segment(task_id))).await
}

/// Scheduler-Zustand einer Aufgabe.
pub async fn scheduler_state(task_id: &str) -> Result<SchedulerStateData, ApiError> {
    get_json(&format!("/api/scheduler-state/{}", encode_segment(task_id))).await
}

/// Amtliche Statistik.
pub async fn officials_stats() -> Result<OfficialsData, ApiError> {
    get_json("/api/officials-stats").await
}

/// Morgenbericht.
pub async fn morning_brief() -> Result<MorningBrief, ApiError> {
    get_json("/api/morning-brief").await
}

/// Agentenkonfiguration.
pub async fn agent_config() -> Result<AgentConfig, ApiError> {
    get_json("/api/agent-config").await
}

/// Modellwechsel-Protokoll.
pub async fn model_change_log() -> Result<Vec<ChangeLogEntry>, ApiError> {
    get_json("/api/model-change-log").await
}

/// Ergebnis des letzten Modellwechsels.
pub async fn last_result() -> Result<Map<String, Value>, ApiError> {
    get_json("/api/last-result").await
}

/// Schreibt den Zustand einer Aufgabe fort.
pub async fn advance_state(task_id: &str, comment: &str) -> Result<ActionResult, ApiError> {
    post_action(
        "/api/advance-state",
        &json!({ "taskId": task_id, "comment": comment }),
    )
    .await
}

/// Archiviert eine Aufgabe oder holt sie zurueck.
pub async fn archive_task(task_id: &str, archived: bool) -> Result<ActionResult, ApiError> {
    post_action(
        "/api/archive-task",
        &json!({ "taskId": task_id, "archived": archived }),
    )
    .await
}

/// Archiviert alle abgeschlossenen Aufgaben.
pub async fn archive_all_done() -> Result<ActionResult, ApiError> {
    post_action("/api/archive-task", &json!({ "archiveAllDone": true })).await
}

/// Speichert die Teilaufgaben einer Aufgabe.
pub async fn update_todos(task_id: &str, todos: &[TodoItem]) -> Result<ActionResult, ApiError> {
    let items: Vec<Value> = todos
        .iter()
        .map(|todo| {
            json!({
                "id": todo.id,
                "title": todo.title,
                "status": todo.status,
            })
        })
        .collect();
    post_action(
        "/api/task-todos",
        &json!({ "taskId": task_id, "todos": items }),
    )
    .await
}

/// Stoppt, bricht ab oder setzt eine Aufgabe fort.
pub async fn task_action(
    task_id: &str,
    action: &str,
    reason: &str,
) -> Result<ActionResult, ApiError> {
    post_action(
        "/api/task-action",
        &json!({ "taskId": task_id, "action": action, "reason": reason }),
    )
    .await
}

/// nimmt eine Aufgabe ab oder lehnt sie ab.
pub async fn review_action(
    task_id: &str,
    action: &str,
    comment: &str,
) -> Result<ActionResult, ApiError> {
    post_action(
        "/api/review-action",
        &json!({ "taskId": task_id, "action": action, "comment": comment }),
    )
    .await
}

/// Weckt einen Agenten auf.
pub async fn agent_wake(agent_id: &str, message: &str) -> Result<ActionResult, ApiError> {
    post_action(
        "/api/agent-wake",
        &json!({ "agentId": agent_id, "message": message }),
    )
    .await
}

/// Legt eine neue Aufgabe an.
pub async fn create_task(task: &NewTask) -> Result<ActionResult, ApiError> {
    let mut body = json!({
        "title": task.title,
        "description": task.description,
        "priority": task.priority,
        "targetDept": task.target_dept,
    });
    // Ohne eigene Organisation setzt der Server seinen Standardwert.
    if !task.org.trim().is_empty() {
        body["org"] = json!(task.org);
    }
    post_action("/api/create-task", &body).await
}

/// Wechselt das Modell eines Agenten.
pub async fn set_model(agent_id: &str, model: &str) -> Result<ActionResult, ApiError> {
    post_action(
        "/api/set-model",
        &json!({ "agentId": agent_id, "model": model }),
    )
    .await
}

/// Eine Nachricht des Servers im WebSocket.
#[derive(Debug, Clone, PartialEq)]
pub enum ServerMessage {
    /// Ein Ereignis mit Thema und Nutzlast.
    Event { topic: String, data: Value },
    /// Antwort auf einen Ping.
    Pong,
    /// Antwort auf eine Anmeldung.
    Subscribed { topics: Vec<String> },
    /// Unbekannte Nachricht.
    Other,
}

/// Baut die WebSocket-Adresse des Servers.
pub fn websocket_url() -> Result<String, ApiError> {
    if !API_BASE.is_empty() {
        let base = API_BASE.trim_end_matches('/');
        if let Some(rest) = base.strip_prefix("https://") {
            return Ok(format!("wss://{rest}/ws"));
        }
        if let Some(rest) = base.strip_prefix("http://") {
            return Ok(format!("ws://{rest}/ws"));
        }
        return Err(ApiError::new(
            "Die WebSocket-Adresse laesst sich aus der Serveradresse nicht ableiten.",
        ));
    }
    let window = web_sys::window().ok_or_else(|| {
        ApiError::new(
            "Der Browserkontext ist nicht verfuegbar. Die WebSocket-Adresse bleibt unbekannt.",
        )
    })?;
    let location = window.location();
    let protocol = location.protocol().map_err(|_| {
        ApiError::new(
            "Die Adresse der Seite liess sich nicht lesen. Die WebSocket-Adresse bleibt unbekannt.",
        )
    })?;
    let host = location.host().map_err(|_| {
        ApiError::new(
            "Die Adresse der Seite liess sich nicht lesen. Die WebSocket-Adresse bleibt unbekannt.",
        )
    })?;
    let scheme = if protocol == "https:" { "wss:" } else { "ws:" };
    Ok(format!("{scheme}//{host}/ws"))
}

/// Baut den Ping-Befehl fuer das WebSocket.
pub fn ping_message() -> String {
    json!({ "type": "ping" }).to_string()
}

/// Baut den Anmeldebefehl fuer das WebSocket.
pub fn subscribe_message(topics: &[&str]) -> String {
    json!({ "type": "subscribe", "topics": topics }).to_string()
}

/// Liest eine Nachricht des Servers.
pub fn parse_server_message(text: &str) -> Option<ServerMessage> {
    let value: Value = serde_json::from_str(text).ok()?;
    match value.get("type").and_then(Value::as_str) {
        Some("event") => Some(ServerMessage::Event {
            topic: value
                .get("topic")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            data: value.get("data").cloned().unwrap_or(Value::Null),
        }),
        Some("pong") => Some(ServerMessage::Pong),
        Some("subscribed") => {
            let topics = value
                .get("topics")
                .and_then(Value::as_array)
                .map(|list| {
                    list.iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default();
            Some(ServerMessage::Subscribed { topics })
        }
        _ => Some(ServerMessage::Other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::format_number;

    #[test]
    fn pfadsegmente_werden_kodiert() {
        assert_eq!(encode_segment("JJC-2026/1"), "JJC-2026%2F1");
        assert_eq!(encode_segment("abc"), "abc");
    }

    #[test]
    fn nachrichten_werden_ausgewertet() {
        let event =
            parse_server_message(r#"{"type":"event","topic":"task.status","data":{"id":"JJC-1"}}"#)
                .expect("Ereignis wird gelesen");
        assert_eq!(
            event,
            ServerMessage::Event {
                topic: "task.status".to_string(),
                data: json!({ "id": "JJC-1" }),
            }
        );
        assert_eq!(
            parse_server_message(r#"{"type":"pong"}"#),
            Some(ServerMessage::Pong)
        );
        assert_eq!(
            parse_server_message(r#"{"type":"subscribed","topics":["task.status"]}"#),
            Some(ServerMessage::Subscribed {
                topics: vec!["task.status".to_string()]
            })
        );
    }

    #[test]
    fn zahlen_werden_gruppiert() {
        assert_eq!(format_number(1234567), "1.234.567");
        assert_eq!(format_number(-42), "-42");
    }
}
