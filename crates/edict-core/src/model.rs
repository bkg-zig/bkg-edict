//! Aufgaben-, Ereignis- und Begleitmodelle.
//!
//! Die JSON-Feldnamen entsprechen dem bestehenden Vertrag des
//! Dashboard-Servers, damit bestehende Clients und die Oberfläche ohne
//! Änderung weiterarbeiten können.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Zustand einer Aufgabe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum TaskState {
    #[serde(rename = "Taizi", alias = "taizi")]
    Taizi,
    #[serde(rename = "Zhongshu", alias = "zhongshu")]
    Zhongshu,
    #[serde(rename = "Menxia", alias = "menxia")]
    Menxia,
    #[serde(rename = "Assigned", alias = "assigned")]
    Assigned,
    #[serde(rename = "Next", alias = "next")]
    Next,
    #[serde(rename = "Doing", alias = "doing")]
    Doing,
    #[serde(rename = "Review", alias = "review")]
    Review,
    #[serde(rename = "Done", alias = "done")]
    Done,
    #[serde(rename = "Blocked", alias = "blocked")]
    Blocked,
    #[serde(rename = "Cancelled", alias = "cancelled")]
    Cancelled,
    #[default]
    #[serde(rename = "Pending", alias = "pending")]
    Pending,
}

impl TaskState {
    /// Kanonischer Wert, der in Datenbank und API gespeichert wird.
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskState::Taizi => "Taizi",
            TaskState::Zhongshu => "Zhongshu",
            TaskState::Menxia => "Menxia",
            TaskState::Assigned => "Assigned",
            TaskState::Next => "Next",
            TaskState::Doing => "Doing",
            TaskState::Review => "Review",
            TaskState::Done => "Done",
            TaskState::Blocked => "Blocked",
            TaskState::Cancelled => "Cancelled",
            TaskState::Pending => "Pending",
        }
    }

    /// Anzeigename auf Deutsch.
    pub fn label(&self) -> &'static str {
        crate::labels::state_label(*self)
    }
}

impl std::fmt::Display for TaskState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for TaskState {
    type Err = crate::AppError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let normalized = value.trim();
        for state in [
            TaskState::Taizi,
            TaskState::Zhongshu,
            TaskState::Menxia,
            TaskState::Assigned,
            TaskState::Next,
            TaskState::Doing,
            TaskState::Review,
            TaskState::Done,
            TaskState::Blocked,
            TaskState::Cancelled,
            TaskState::Pending,
        ] {
            if state.as_str().eq_ignore_ascii_case(normalized) {
                return Ok(state);
            }
        }
        Err(crate::AppError::InvalidState(value.to_string()))
    }
}

/// Eintrag im Zustandsprotokoll einer Aufgabe.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FlowEntry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remark: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ts: Option<String>,
}

/// Eintrag im Fortschrittsprotokoll einer Aufgabe.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProgressEntry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(
        default,
        rename = "agentLabel",
        skip_serializing_if = "Option::is_none"
    )]
    pub agent_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub todos: Vec<TodoItem>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub org: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elapsed: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ts: Option<String>,
}

/// Teilaufgabe einer Aufgabe.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TodoItem {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default = "default_todo_status")]
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

fn default_todo_status() -> String {
    "not-started".to_string()
}

/// Status eines Teilpunkts.
pub const TODO_STATUSES: [&str; 3] = ["not-started", "in-progress", "completed"];

/// Herzschlag einer Aufgabe, abgeleitet aus dem letzten Aktualisierungszeitpunkt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Heartbeat {
    pub status: String,
    pub label: String,
    #[serde(rename = "ageSec", skip_serializing_if = "Option::is_none")]
    pub age_sec: Option<i64>,
}

/// Metadaten des Scheduler-Zustands einer Aufgabe.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SchedulerMeta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_scan: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_progress: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_count: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub escalated: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stalled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review_round: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rolled_back_from: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Eine Aufgabe des Systems.
///
/// Drei Kennungen werden getrennt geführt und nicht zusammengeführt:
/// `task_id` ist die technische UUID und dient als Primärschlüssel,
/// `id` ist die fachliche Legacy-Kennung wie `JJC-20260301-001`, und
/// `trace_id` verbindet die Ereigniskette einer Aufgabe.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Task {
    /// Technische Kennung als UUID. Primärschlüssel in PostgreSQL.
    #[serde(default)]
    pub task_id: String,
    /// Stabile fachliche Kennung, im Legacy-Betrieb `JJC-YYYYMMDD-NNN`.
    pub id: String,
    /// Technische Kennung, die Ereignisse und Worker verbindet.
    #[serde(default)]
    pub trace_id: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub state: TaskState,
    #[serde(default)]
    pub org: String,
    #[serde(default)]
    pub official: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assignee_org: Option<String>,
    #[serde(default)]
    pub creator: String,
    #[serde(default)]
    pub now: String,
    #[serde(default)]
    pub eta: String,
    #[serde(default)]
    pub block: String,
    #[serde(default)]
    pub output: String,
    #[serde(default)]
    pub ac: String,
    #[serde(default)]
    pub priority: String,
    #[serde(default)]
    pub archived: bool,
    #[serde(default)]
    pub flow_log: Vec<FlowEntry>,
    #[serde(default)]
    pub progress_log: Vec<ProgressEntry>,
    #[serde(default)]
    pub todos: Vec<TodoItem>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub meta: Map<String, Value>,
    #[serde(rename = "templateId", default)]
    pub template_id: String,
    #[serde(rename = "templateParams", default)]
    pub template_params: Map<String, Value>,
    #[serde(rename = "targetDept", default)]
    pub target_dept: String,
    #[serde(rename = "_scheduler", default)]
    pub scheduler: Map<String, Value>,
    #[serde(rename = "review_round", default)]
    pub review_round: i64,
    #[serde(rename = "createdAt", default)]
    pub created_at: String,
    #[serde(rename = "updatedAt", default)]
    pub updated_at: String,
    #[serde(
        rename = "archivedAt",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub archived_at: Option<String>,
    #[serde(
        rename = "sourceMeta",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub source_meta: Option<Map<String, Value>>,
    #[serde(
        rename = "_prev_state",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub prev_state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heartbeat: Option<Heartbeat>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub activity: Vec<Value>,
    /// Optimistische Sperre. Jede Speicherung erhöht den Zähler um eins.
    #[serde(default)]
    pub version: i64,
}

impl Task {
    /// Neuer Zustand mit Standardwerten.
    ///
    /// Eine neue Aufgabe startet beim Taizi und trägt den ersten Eintrag im
    /// Zustandsprotokoll. Die Texte sind deutsch, damit die Datenbank keine
    /// chinesischen und deutschen Werte mischt.
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        let now = Utc::now().to_rfc3339();
        Task {
            task_id: uuid::Uuid::new_v4().to_string(),
            id: id.into(),
            trace_id: uuid::Uuid::new_v4().to_string(),
            title: title.into(),
            state: TaskState::Taizi,
            org: "Taizi".to_string(),
            creator: "Kaiser".to_string(),
            now: "Wartet auf Sortierung durch den Taizi".to_string(),
            eta: "-".to_string(),
            block: "Keine".to_string(),
            priority: "normal".to_string(),
            flow_log: vec![FlowEntry {
                at: Some(now.clone()),
                from: Some("Kaiser".to_string()),
                to: Some("Taizi".to_string()),
                remark: Some("Aufgabe erstellt".to_string()),
                agent: Some("Kaiser".to_string()),
                reason: Some("Aufgabe erstellt".to_string()),
                ts: Some(now.clone()),
            }],
            created_at: now.clone(),
            updated_at: now,
            ..Default::default()
        }
    }

    /// Aktualisiert den Zeitstempel der letzten Änderung.
    pub fn touch(&mut self) {
        self.updated_at = Utc::now().to_rfc3339();
    }

    /// Berechnet den Herzschlag anhand des Alters der letzten Änderung.
    pub fn compute_heartbeat(&mut self, now: DateTime<Utc>) {
        if !crate::state_machine::is_active_state(self.state) {
            self.heartbeat = None;
            return;
        }
        let updated = self.updated_at.parse::<DateTime<Utc>>().ok().or_else(|| {
            self.source_meta
                .as_ref()
                .and_then(|meta| meta.get("updatedAt"))
                .and_then(|value| value.as_str())
                .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
                .map(|value| value.with_timezone(&Utc))
        });
        match updated {
            None => {
                self.heartbeat = Some(Heartbeat {
                    status: "unknown".to_string(),
                    label: "⚪ Unbekannt".to_string(),
                    age_sec: None,
                });
            }
            Some(updated) => {
                let age = (now - updated).num_seconds();
                let minutes = age / 60;
                let (status, label) = if age < 180 {
                    ("active", format!("🟢 Aktiv, vor {minutes} Minuten"))
                } else if age < 600 {
                    (
                        "warn",
                        format!("🟡 Möglicherweise festgefahren, vor {minutes} Minuten"),
                    )
                } else {
                    ("stalled", format!("🔴 Festgefahren seit {minutes} Minuten"))
                };
                self.heartbeat = Some(Heartbeat {
                    status: status.to_string(),
                    label,
                    age_sec: Some(age),
                });
            }
        }
    }
}

/// Ein in PostgreSQL gespeichertes Ereignis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    #[serde(rename = "event_id")]
    pub event_id: String,
    pub trace_id: String,
    pub timestamp: String,
    pub topic: String,
    pub event_type: String,
    pub producer: String,
    pub payload: Map<String, Value>,
    pub meta: Map<String, Value>,
}

/// Live-Status der Aufgabenliste.
///
/// Die Feldnamen entsprechen dem Vertrag des bisherigen Dashboard-Servers.
/// `tasks` ist ein Array, nicht eine Map. Die Felder `generatedAt`,
/// `taskSource`, `officials`, `history`, `metrics` und `health` sind optional,
/// damit die Oberfläche nur `tasks` und `syncStatus` benötigt, die Dashboard-
///Ansicht aber die vollständigen Angaben behalten kann.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveStatus {
    pub tasks: Vec<Task>,
    #[serde(rename = "syncStatus")]
    pub sync_status: Map<String, Value>,
    #[serde(
        rename = "generatedAt",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub generated_at: Option<String>,
    #[serde(
        rename = "taskSource",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub task_source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub officials: Option<Vec<Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub history: Option<Vec<Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metrics: Option<Map<String, Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub health: Option<Map<String, Value>>,
}

/// Themen des Ereignisbus.
pub mod topics {
    pub const TASK_CREATED: &str = "task.created";
    pub const TASK_PLANNING_REQUEST: &str = "task.planning.request";
    pub const TASK_PLANNING_COMPLETE: &str = "task.planning.complete";
    pub const TASK_REVIEW_REQUEST: &str = "task.review.request";
    pub const TASK_REVIEW_RESULT: &str = "task.review.result";
    pub const TASK_DISPATCH: &str = "task.dispatch";
    pub const TASK_STATUS: &str = "task.status";
    pub const TASK_COMPLETED: &str = "task.completed";
    pub const TASK_CLOSED: &str = "task.closed";
    pub const TASK_REPLAN: &str = "task.replan";
    pub const TASK_SCHEDULER: &str = "task.scheduler";
    pub const TASK_STALLED: &str = "task.stalled";
    pub const TASK_ESCALATED: &str = "task.escalated";
    pub const AGENT_THOUGHTS: &str = "agent.thoughts";
    pub const AGENT_TODO_UPDATE: &str = "agent.todo.update";
    pub const AGENT_HEARTBEAT: &str = "agent.heartbeat";
}

/// Präfix der Redis-Streams.
pub const STREAM_PREFIX: &str = "edict:stream:";

/// Präfix der Redis-Pub/Sub-Kanäle.
pub const PUBSUB_PREFIX: &str = "edict:pubsub:";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zustandswerte_sind_case_insensitiv_lesbar() {
        assert_eq!("taizi".parse::<TaskState>().unwrap(), TaskState::Taizi);
        assert_eq!("Doing".parse::<TaskState>().unwrap(), TaskState::Doing);
        assert!("ungueltig".parse::<TaskState>().is_err());
    }

    #[test]
    fn aufgabe_serialisiert_legacy_feldnamen() {
        let task = Task::new("JJC-20260301-001", "Test");
        let json = serde_json::to_value(&task).unwrap();
        assert_eq!(json["id"], "JJC-20260301-001");
        assert!(json.get("templateId").is_some());
        assert!(json.get("createdAt").is_some());
        assert!(json.get("flow_log").is_some());
    }

    #[test]
    fn heartbeat_ist_nur_fuer_aktive_zustaende_vorhanden() {
        let mut task = Task::new("JJC-1", "Test");
        let now = Utc::now();
        task.state = TaskState::Done;
        task.compute_heartbeat(now);
        assert!(task.heartbeat.is_none());

        task.state = TaskState::Doing;
        task.compute_heartbeat(now);
        assert_eq!(task.heartbeat.unwrap().status, "active");
    }
}
