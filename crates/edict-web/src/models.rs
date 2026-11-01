//! Datenmodelle und deutsche Anzeigetexte der Weboberflaeche.
//!
//! Die Modelle bilden den JSON-Vertrag des Dashboard-Servers ab. Sie
//! spiegeln `edict-core::model`, `edict-core::labels` und
//! `edict-core::state_machine`, weil `edict-core` wegen der Dateisperre in
//! `fs2` nicht fuer `wasm32-unknown-unknown` gebaut werden kann. Die
//! README nennt den Compilerfehler und die Folgen.

use std::collections::BTreeMap;

use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Zustand einer Aufgabe. Die kanonischen Werte entsprechen `edict-core`.
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
    /// Kanonischer Wert, der in Datenbank und API steht.
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

    /// Deutscher Anzeigename.
    pub fn label(&self) -> &'static str {
        state_label(*self)
    }

    /// Liest einen kanonischen Wert, unabhaengig von der Schreibweise.
    pub fn parse(value: &str) -> Option<TaskState> {
        let normalized = value.trim();
        TaskState::ALL
            .iter()
            .find(|state| state.as_str().eq_ignore_ascii_case(normalized))
            .copied()
    }

    /// Zustaende, die als abgeschlossen gelten.
    pub fn is_terminal(&self) -> bool {
        matches!(self, TaskState::Done | TaskState::Cancelled)
    }

    /// Alle Zustaende in kanonischer Reihenfolge.
    pub const ALL: [TaskState; 11] = [
        TaskState::Pending,
        TaskState::Taizi,
        TaskState::Zhongshu,
        TaskState::Menxia,
        TaskState::Assigned,
        TaskState::Next,
        TaskState::Doing,
        TaskState::Review,
        TaskState::Blocked,
        TaskState::Done,
        TaskState::Cancelled,
    ];
}

impl std::fmt::Display for TaskState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Deutscher Name eines Zustands.
pub fn state_label(state: TaskState) -> &'static str {
    match state {
        TaskState::Taizi => "Taizi",
        TaskState::Zhongshu => "Zhongshu",
        TaskState::Menxia => "Menxia",
        TaskState::Assigned => "Zugewiesen",
        TaskState::Next => "Bereit",
        TaskState::Doing => "In Arbeit",
        TaskState::Review => "Prüfung",
        TaskState::Done => "Abgeschlossen",
        TaskState::Blocked => "Blockiert",
        TaskState::Cancelled => "Abgebrochen",
        TaskState::Pending => "Offen",
    }
}

/// Deutscher Name einer Prioritaet.
pub fn priority_label(priority: &str) -> &'static str {
    match priority.trim().to_ascii_lowercase().as_str() {
        "low" | "niedrig" => "Niedrig",
        "high" | "hoch" => "Hoch",
        "urgent" | "dringend" => "Dringend",
        "中" | "mittel" => "Mittel",
        _ => "Normal",
    }
}

/// Kanonische Organisationsbezeichnungen und ihre deutschen Namen.
pub const ORG_LABELS: &[(&str, &str)] = &[
    ("中书省", "Zhongshu"),
    ("门下省", "Menxia"),
    ("尚书省", "Shangshu"),
    ("六部", "Sechs Ministerien"),
    ("礼部", "Rites-Ministerium"),
    ("兵部", "Kriegsministerium"),
    ("户部", "Finanzministerium"),
    ("刑部", "Justizministerium"),
    ("工部", "Bauministerium"),
    ("吏部", "Personalministerium"),
    ("钦天监", "Astronomisches Amt"),
    ("皇上", "Kaiser"),
    ("太子", "Taizi"),
    ("执行中", "In Arbeit"),
];

/// Deutscher Name einer Organisation.
pub fn org_label(value: &str) -> String {
    ORG_LABELS
        .iter()
        .find(|(source, _)| *source == value)
        .map(|(_, target)| (*target).to_string())
        .unwrap_or_else(|| value.to_string())
}

/// Kanonische Amtsbezeichnungen und ihre deutschen Namen.
pub const OFFICIAL_LABELS: &[(&str, &str)] = &[
    ("中书令", "Leiter der Zhongshu-Kanzlei"),
    ("侍中", "Palastprüfer"),
    ("尚书令", "Leiter der Shangshu-Kanzlei"),
    ("吏部尚书", "Personalminister"),
    ("户部尚书", "Finanzminister"),
    ("礼部尚书", "Rites-Minister"),
    ("兵部尚书", "Kriegsminister"),
    ("刑部尚书", "Justizminister"),
    ("工部尚书", "Bauminister"),
];

/// Deutsche Bezeichnungen der chinesischen Zustandsbezeichnungen des Servers.
pub const STATE_LABELS_SERVER: &[(&str, &str)] = &[
    ("待处理", "Offen"),
    ("太子", "Taizi"),
    ("中书省", "Zhongshu"),
    ("门下省", "Menxia"),
    ("尚书省", "Shangshu"),
    ("待执行", "Bereit"),
    ("执行中", "In Arbeit"),
    ("审查", "Pruefung"),
    ("完成", "Abgeschlossen"),
];

/// Deutscher Name des Versandstatus des Schedulers.
pub fn dispatch_status_label(value: &str) -> &'static str {
    match value {
        "idle" => "Bereit",
        "queued" => "In der Warteschlange",
        "success" => "Erfolgreich",
        "failed" => "Fehlgeschlagen",
        "timeout" => "Zeitueberschreitung",
        "gateway-offline" => "Gateway offline",
        "error" => "Fehler",
        _ => "Unbekannt",
    }
}

/// Deutscher Name einer chinesischen Zustandsbezeichnung des Servers.
pub fn server_state_label(value: &str) -> String {
    STATE_LABELS_SERVER
        .iter()
        .find(|(source, _)| *source == value)
        .map(|(_, target)| (*target).to_string())
        .unwrap_or_else(|| value.to_string())
}

/// Deutscher Name eines Amtsinhabers.
pub fn official_label(value: &str) -> String {
    OFFICIAL_LABELS
        .iter()
        .find(|(source, _)| *source == value)
        .map(|(_, target)| (*target).to_string())
        .unwrap_or_else(|| value.to_string())
}

/// Wandelt eine Zahl in eine lesbare deutsche Angabe.
pub fn format_number(value: i64) -> String {
    let text = value.abs().to_string();
    let mut grouped = String::new();
    for (index, digit) in text.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            grouped.push('.');
        }
        grouped.push(digit);
    }
    let grouped = grouped.chars().rev().collect::<String>();
    if value < 0 {
        format!("-{grouped}")
    } else {
        grouped
    }
}

/// Wandelt einen Betrag in eine Euro-Angabe.
pub fn format_currency(value: f64) -> String {
    format!("{value:.2} EUR")
}

/// Kuerzt einen Text auf eine maximale Laenge.
pub fn truncate(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut shortened: String = text.chars().take(max_chars.saturating_sub(1)).collect();
    shortened.push('…');
    shortened
}

/// Spalten des Boards. Die Reihenfolge folgt `BOARD_ORDER` aus `edict-core`
/// und ergaenzt die drei dort nicht enthaltenen Zustaende.
pub const BOARD_COLUMNS: [TaskState; 11] = [
    TaskState::Pending,
    TaskState::Taizi,
    TaskState::Zhongshu,
    TaskState::Menxia,
    TaskState::Assigned,
    TaskState::Next,
    TaskState::Doing,
    TaskState::Review,
    TaskState::Blocked,
    TaskState::Done,
    TaskState::Cancelled,
];

/// Stationen der Prozesskette, die in der Aufgabenkarte angezeigt werden.
pub const PIPELINE: [TaskState; 8] = [
    TaskState::Pending,
    TaskState::Taizi,
    TaskState::Zhongshu,
    TaskState::Menxia,
    TaskState::Assigned,
    TaskState::Doing,
    TaskState::Review,
    TaskState::Done,
];

/// Position eines Zustands in der Prozesskette.
pub fn pipeline_index(state: TaskState) -> usize {
    match state {
        TaskState::Next => 4,
        TaskState::Blocked => 5,
        TaskState::Cancelled => 7,
        other => PIPELINE.iter().position(|step| *step == other).unwrap_or(4),
    }
}

/// Eintrag im Zustandsprotokoll einer Aufgabe.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct FlowEntry {
    #[serde(default)]
    pub at: Option<String>,
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub to: Option<String>,
    #[serde(default)]
    pub remark: Option<String>,
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub ts: Option<String>,
}

/// Eintrag im Fortschrittsprotokoll einer Aufgabe.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ProgressEntry {
    #[serde(default)]
    pub at: Option<String>,
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(rename = "agentLabel", default)]
    pub agent_label: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub todos: Vec<TodoItem>,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub org: Option<String>,
    #[serde(default)]
    pub tokens: Option<i64>,
    #[serde(default)]
    pub cost: Option<f64>,
    #[serde(default)]
    pub elapsed: Option<i64>,
    #[serde(default)]
    pub ts: Option<String>,
}

/// Teilaufgabe einer Aufgabe.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TodoItem {
    #[serde(default, deserialize_with = "deserialize_id")]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default = "default_todo_status")]
    pub status: String,
    #[serde(default)]
    pub detail: Option<String>,
}

fn default_todo_status() -> String {
    "not-started".to_string()
}

/// Liest eine Teilaufgaben-Kennung, die im Legacy als Zahl oder Text vorliegt.
fn deserialize_id<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    Ok(match value {
        Value::String(text) => text,
        Value::Number(number) => number.to_string(),
        _ => String::new(),
    })
}

/// Zulaessige Statuswerte einer Teilaufgabe.
pub const TODO_STATUSES: [&str; 3] = ["not-started", "in-progress", "completed"];

/// Deutscher Name eines Teilaufgaben-Status.
pub fn todo_status_label(status: &str) -> &'static str {
    match status {
        "in-progress" => "In Arbeit",
        "completed" => "Erledigt",
        _ => "Offen",
    }
}

/// Herzschlag einer Aufgabe.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Heartbeat {
    #[serde(default)]
    pub status: String,
    /// Beschriftung des Servers. Sie ist nichtsprachig und wird nicht angezeigt.
    #[serde(default)]
    pub label: String,
    #[serde(rename = "ageSec", default)]
    pub age_sec: Option<i64>,
}

/// Deutscher Text zum Herzschlag einer Aufgabe.
///
/// Der Server liefert eine chinesische Beschriftung. Die Oberflaeche zeigt
/// stattdessen den kanonischen Status und das Alter in Sekunden oder Minuten.
pub fn heartbeat_label(heartbeat: &Heartbeat) -> String {
    let status = match heartbeat.status.trim().to_ascii_lowercase().as_str() {
        "active" => "Aktiv",
        "warn" => "Moeglicherweise stillstehend",
        "stalled" => "Stillstehend",
        "unknown" => "Unbekannt",
        _ => "Unbekannt",
    };
    match heartbeat.age_sec {
        Some(age) if age < 60 => format!("{status}, vor {age} Sekunden"),
        Some(age) => format!("{status}, vor {} Minuten", age / 60),
        None => status.to_string(),
    }
}

/// Eine Aufgabe des Systems.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Task {
    #[serde(default)]
    pub task_id: String,
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub trace_id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub state: TaskState,
    #[serde(default)]
    pub org: String,
    #[serde(default)]
    pub official: String,
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
    pub review_round: i64,
    #[serde(rename = "createdAt", default)]
    pub created_at: String,
    #[serde(rename = "updatedAt", default)]
    pub updated_at: String,
    #[serde(rename = "archivedAt", default)]
    pub archived_at: Option<String>,
    #[serde(rename = "sourceMeta", default)]
    pub source_meta: Option<Map<String, Value>>,
    #[serde(rename = "_prev_state", default)]
    pub prev_state: Option<String>,
    #[serde(default)]
    pub heartbeat: Option<Heartbeat>,
    #[serde(default)]
    pub activity: Vec<Value>,
    #[serde(rename = "templateId", default)]
    pub template_id: String,
    #[serde(rename = "targetDept", default)]
    pub target_dept: String,
}

impl Task {
    /// Fachliche Kennung fuer Anzeigen und Protokolle.
    pub fn display_id(&self) -> &str {
        if self.id.is_empty() {
            &self.task_id
        } else {
            &self.id
        }
    }

    /// Zustand mit Pruefrunde, wie ihn die Karten und die Detailansicht zeigen.
    pub fn state_text(&self) -> String {
        let base = state_label(self.state).to_string();
        match (self.state, self.review_round) {
            (TaskState::Menxia, round) if round > 1 => format!("{base} (Runde {round})"),
            (TaskState::Zhongshu, round) if round > 0 => format!("{base} (Runde {round})"),
            (TaskState::Review, round) if round > 0 => format!("{base} (Runde {round})"),
            _ => base,
        }
    }

    /// Eine Aufgabe gilt als archiviert, wenn sie archiviert oder beendet ist.
    pub fn is_archived(&self) -> bool {
        self.archived || self.state.is_terminal()
    }

    /// Anteil der erledigten Teilaufgaben in Prozent.
    pub fn todo_progress(&self) -> u8 {
        if self.todos.is_empty() {
            return 0;
        }
        let done = self
            .todos
            .iter()
            .filter(|todo| todo.status == "completed")
            .count();
        ((done * 100) / self.todos.len()) as u8
    }

    /// Der zustaendige Agent der Aufgabe, falls der Zustand einen benennt.
    pub fn responsible_agent(&self) -> Option<&'static str> {
        match self.state {
            TaskState::Taizi => Some("taizi"),
            TaskState::Zhongshu => Some("zhongshu"),
            TaskState::Menxia => Some("menxia"),
            TaskState::Assigned | TaskState::Review => Some("shangshu"),
            _ => None,
        }
    }
}

/// Live-Status der Aufgabenliste.
#[derive(Debug, Clone, PartialEq, Deserialize, Default)]
pub struct LiveStatus {
    #[serde(default)]
    pub tasks: Vec<Task>,
    #[serde(rename = "syncStatus", default)]
    pub sync_status: Map<String, Value>,
    #[serde(rename = "generatedAt", default)]
    pub generated_at: Option<String>,
    #[serde(rename = "taskSource", default)]
    pub task_source: Option<String>,
    #[serde(default)]
    pub health: Option<Map<String, Value>>,
}

/// Antwort auf eine schreibende Aktion.
#[derive(Debug, Clone, PartialEq, Deserialize, Default)]
pub struct ActionResult {
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(rename = "taskId", default)]
    pub task_id: Option<String>,
    #[serde(default)]
    pub count: Option<i64>,
}

/// Zustand des Gateways in der Agentenuebersicht.
#[derive(Debug, Clone, PartialEq, Deserialize, Default)]
pub struct GatewayStatus {
    #[serde(default)]
    pub alive: bool,
    #[serde(default)]
    pub probe: bool,
    #[serde(default)]
    pub status: String,
}

/// Status eines Agenten.
#[derive(Debug, Clone, PartialEq, Deserialize, Default)]
pub struct AgentStatusInfo {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub emoji: String,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub status: String,
    #[serde(rename = "statusLabel", default)]
    pub status_label: String,
    #[serde(rename = "lastActive", default)]
    pub last_active: Option<String>,
}

/// Antwort der Agentenstatus-Schnittstelle.
#[derive(Debug, Clone, PartialEq, Deserialize, Default)]
pub struct AgentsStatusData {
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub gateway: GatewayStatus,
    #[serde(default)]
    pub agents: Vec<AgentStatusInfo>,
    #[serde(rename = "checkedAt", default)]
    pub checked_at: String,
}

/// Deutscher Name eines Agentenstatus.
pub fn agent_status_label(status: &str) -> &'static str {
    match status {
        "running" => "Läuft",
        "idle" => "Bereit",
        "offline" => "Offline",
        "unconfigured" => "Nicht eingerichtet",
        _ => "Unbekannt",
    }
}

/// Beteiligung eines Amtsinhabers an einer Aufgabe.
#[derive(Debug, Clone, PartialEq, Deserialize, Default)]
pub struct ParticipatedEdict {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub state: String,
}

/// Kennzahlen eines Amtsinhabers.
#[derive(Debug, Clone, PartialEq, Deserialize, Default)]
pub struct OfficialInfo {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub emoji: String,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub rank: String,
    #[serde(default)]
    pub model: String,
    #[serde(rename = "model_short", default)]
    pub model_short: String,
    #[serde(default)]
    pub tokens_in: i64,
    #[serde(default)]
    pub tokens_out: i64,
    #[serde(default)]
    pub cache_read: i64,
    #[serde(default)]
    pub cache_write: i64,
    #[serde(default)]
    pub cost_cny: f64,
    #[serde(default)]
    pub cost_usd: f64,
    #[serde(default)]
    pub sessions: i64,
    #[serde(default)]
    pub messages: i64,
    #[serde(default)]
    pub tasks_done: i64,
    #[serde(default)]
    pub tasks_active: i64,
    #[serde(default)]
    pub flow_participations: i64,
    #[serde(default)]
    pub merit_score: f64,
    #[serde(default)]
    pub merit_rank: i64,
    #[serde(rename = "last_active", default)]
    pub last_active: String,
    #[serde(default)]
    pub heartbeat: Option<Heartbeat>,
    #[serde(default)]
    pub participated_edicts: Vec<ParticipatedEdict>,
}

/// Summen der amtlichen Statistik.
#[derive(Debug, Clone, PartialEq, Deserialize, Default)]
pub struct OfficialTotals {
    #[serde(default)]
    pub tasks_done: i64,
    #[serde(default)]
    pub cost_cny: f64,
}

/// Antwort der amtlichen Statistik.
#[derive(Debug, Clone, PartialEq, Deserialize, Default)]
pub struct OfficialsData {
    #[serde(default)]
    pub officials: Vec<OfficialInfo>,
    #[serde(default)]
    pub totals: OfficialTotals,
    #[serde(rename = "top_official", default)]
    pub top_official: String,
}

/// Ein Eintrag des Morgenberichts.
#[derive(Debug, Clone, PartialEq, Deserialize, Default)]
pub struct MorningNewsItem {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub desc: Option<String>,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(rename = "pub_date", alias = "time", default)]
    pub pub_date: Option<String>,
    #[serde(rename = "url", alias = "link", default)]
    pub url: String,
}

/// Der Morgenbericht mit seinen Kategorien.
#[derive(Debug, Clone, PartialEq, Deserialize, Default)]
pub struct MorningBrief {
    #[serde(default)]
    pub date: Option<String>,
    #[serde(rename = "generated_at", default)]
    pub generated_at: Option<String>,
    #[serde(default)]
    pub categories: BTreeMap<String, Vec<MorningNewsItem>>,
}

/// Ein Skill eines Agenten.
#[derive(Debug, Clone, PartialEq, Deserialize, Default)]
pub struct SkillInfo {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub path: String,
}

/// Ein Agent der Konfiguration.
#[derive(Debug, Clone, PartialEq, Deserialize, Default)]
pub struct AgentInfo {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub emoji: String,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub skills: Vec<SkillInfo>,
}

/// Ein bekanntes Modell.
#[derive(Debug, Clone, PartialEq, Deserialize, Default)]
pub struct KnownModel {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub provider: String,
}

/// Die Agentenkonfiguration.
#[derive(Debug, Clone, PartialEq, Deserialize, Default)]
pub struct AgentConfig {
    #[serde(default)]
    pub agents: Vec<AgentInfo>,
    #[serde(rename = "knownModels", default)]
    pub known_models: Vec<KnownModel>,
}

/// Ein Eintrag des Modellwechsel-Protokolls.
#[derive(Debug, Clone, PartialEq, Deserialize, Default)]
pub struct ChangeLogEntry {
    #[serde(default)]
    pub at: String,
    #[serde(rename = "agentId", default)]
    pub agent_id: String,
    #[serde(rename = "oldModel", default)]
    pub old_model: String,
    #[serde(rename = "newModel", default)]
    pub new_model: String,
    #[serde(rename = "rolledBack", default)]
    pub rolled_back: bool,
}

/// Ein Eintrag im Aktivitaetsprotokoll eines Agenten oder einer Aufgabe.
#[derive(Debug, Clone, PartialEq, Deserialize, Default)]
pub struct ActivityEntry {
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub at: Option<Value>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub thinking: Option<String>,
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub to: Option<String>,
    #[serde(default)]
    pub remark: Option<String>,
    #[serde(default)]
    pub tool: Option<String>,
    #[serde(default)]
    pub output: Option<String>,
    #[serde(rename = "exitCode", default)]
    pub exit_code: Option<i64>,
    #[serde(default)]
    pub items: Vec<TodoItem>,
}

/// Antwort der Aktivitaetsschnittstelle eines Agenten.
#[derive(Debug, Clone, PartialEq, Deserialize, Default)]
pub struct AgentActivityData {
    #[serde(default)]
    pub ok: bool,
    #[serde(rename = "agentId", default)]
    pub agent_id: String,
    #[serde(default)]
    pub activity: Vec<ActivityEntry>,
}

/// Dauer einer Phase einer Aufgabe.
#[derive(Debug, Clone, PartialEq, Deserialize, Default)]
pub struct PhaseDuration {
    #[serde(default)]
    pub phase: String,
    #[serde(rename = "durationSec", default)]
    pub duration_sec: i64,
    #[serde(rename = "durationText", default)]
    pub duration_text: String,
    #[serde(default)]
    pub ongoing: bool,
}

/// Zusammenfassung der Teilaufgaben einer Aufgabe.
#[derive(Debug, Clone, PartialEq, Deserialize, Default)]
pub struct TodosSummary {
    #[serde(default)]
    pub total: i64,
    #[serde(default)]
    pub completed: i64,
    #[serde(rename = "inProgress", default)]
    pub in_progress: i64,
    #[serde(rename = "notStarted", default)]
    pub not_started: i64,
    #[serde(default)]
    pub percent: i64,
}

/// Antwort der Aktivitaetsschnittstelle einer Aufgabe.
#[derive(Debug, Clone, PartialEq, Deserialize, Default)]
pub struct TaskActivityData {
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub activity: Vec<ActivityEntry>,
    #[serde(rename = "relatedAgents", default)]
    pub related_agents: Vec<String>,
    #[serde(rename = "agentLabel", default)]
    pub agent_label: Option<String>,
    #[serde(rename = "lastActive", default)]
    pub last_active: Option<String>,
    #[serde(rename = "phaseDurations", default)]
    pub phase_durations: Vec<PhaseDuration>,
    #[serde(rename = "totalDuration", default)]
    pub total_duration: Option<String>,
    #[serde(rename = "todosSummary", default)]
    pub todos_summary: Option<TodosSummary>,
}

/// Scheduler-Metadaten einer Aufgabe.
#[derive(Debug, Clone, PartialEq, Deserialize, Default)]
pub struct SchedulerInfo {
    #[serde(rename = "retryCount", default)]
    pub retry_count: Option<i64>,
    #[serde(rename = "escalationLevel", default)]
    pub escalation_level: Option<i64>,
    #[serde(rename = "lastDispatchStatus", default)]
    pub last_dispatch_status: Option<String>,
    #[serde(rename = "stallThresholdSec", default)]
    pub stall_threshold_sec: Option<i64>,
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(rename = "lastProgressAt", default)]
    pub last_progress_at: Option<String>,
    #[serde(rename = "lastDispatchAt", default)]
    pub last_dispatch_at: Option<String>,
    #[serde(rename = "lastDispatchAgent", default)]
    pub last_dispatch_agent: Option<String>,
    #[serde(rename = "autoRollback", default)]
    pub auto_rollback: Option<bool>,
}

/// Antwort der Scheduler-Schnittstelle.
#[derive(Debug, Clone, PartialEq, Deserialize, Default)]
pub struct SchedulerStateData {
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub scheduler: Option<SchedulerInfo>,
    #[serde(rename = "stalledSec", default)]
    pub stalled_sec: Option<i64>,
}

/// Daten fuer eine neu angelegte Aufgabe.
#[derive(Debug, Clone, PartialEq)]
pub struct NewTask {
    pub title: String,
    pub description: String,
    pub org: String,
    pub priority: String,
    pub target_dept: String,
}

/// Liest einen Zeitstempel im RFC-3339- oder im Legacy-Format.
pub fn parse_timestamp(value: &str) -> Option<DateTime<Utc>> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(parsed) = DateTime::parse_from_rfc3339(trimmed) {
        return Some(parsed.with_timezone(&Utc));
    }
    if let Ok(parsed) = NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%d %H:%M:%S") {
        return Some(parsed.and_utc());
    }
    if let Ok(parsed) = NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%d %H:%M") {
        return Some(parsed.and_utc());
    }
    if let Ok(parsed) = NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%dT%H:%M:%S") {
        return Some(parsed.and_utc());
    }
    None
}

/// Formatiert einen Zeitstempel als deutschen Text.
pub fn format_timestamp(value: &str) -> String {
    match parse_timestamp(value) {
        Some(moment) => moment
            .with_timezone(&Utc)
            .format("%d.%m.%Y, %H:%M Uhr")
            .to_string(),
        None => value.to_string(),
    }
}

/// Formatiert einen Zeitstempel alsrelative Angabe.
pub fn relative_time(value: &str) -> String {
    match parse_timestamp(value) {
        Some(moment) => {
            let seconds = (Utc::now() - moment).num_seconds();
            if seconds < 0 {
                "gleich".to_string()
            } else if seconds < 60 {
                "gerade eben".to_string()
            } else if seconds < 3600 {
                format!("vor {} Minuten", seconds / 60)
            } else if seconds < 86_400 {
                format!("vor {} Stunden", seconds / 3600)
            } else {
                format!("vor {} Tagen", seconds / 86_400)
            }
        }
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bildet die Zustände der Oberflaeche auf die des Fachkerns ab.
    fn edict_core_state(state: TaskState) -> edict_core::TaskState {
        match state {
            TaskState::Taizi => edict_core::TaskState::Taizi,
            TaskState::Zhongshu => edict_core::TaskState::Zhongshu,
            TaskState::Menxia => edict_core::TaskState::Menxia,
            TaskState::Assigned => edict_core::TaskState::Assigned,
            TaskState::Next => edict_core::TaskState::Next,
            TaskState::Doing => edict_core::TaskState::Doing,
            TaskState::Review => edict_core::TaskState::Review,
            TaskState::Done => edict_core::TaskState::Done,
            TaskState::Blocked => edict_core::TaskState::Blocked,
            TaskState::Cancelled => edict_core::TaskState::Cancelled,
            TaskState::Pending => edict_core::TaskState::Pending,
        }
    }

    #[test]
    fn zustaende_entsprechen_dem_fachkern() {
        for state in TaskState::ALL {
            let core = edict_core_state(state);
            assert_eq!(state.as_str(), core.as_str(), "Kanonischer Wert");
            assert_eq!(
                state_label(state),
                edict_core::labels::state_label(core),
                "Deutsche Beschriftung"
            );
        }
    }

    #[test]
    fn abgeschlossene_zustaende_entsprechen_dem_fachkern() {
        for state in TaskState::ALL {
            let core = edict_core_state(state);
            assert_eq!(
                state.is_terminal(),
                edict_core::TERMINAL_STATES.contains(&core),
                "Abschlussstatus"
            );
        }
    }

    #[test]
    fn zustandswerte_werden_case_insensitiv_gelesen() {
        assert_eq!(TaskState::parse("doing"), Some(TaskState::Doing));
        assert_eq!(TaskState::parse("Review"), Some(TaskState::Review));
        assert_eq!(TaskState::parse("unsinn"), None);
    }

    #[test]
    fn aufgabenfelder_entsprechen_legacy_namen() {
        let task = Task {
            id: "JJC-1".to_string(),
            title: "Test".to_string(),
            state: TaskState::Doing,
            ..Default::default()
        };
        let json = serde_json::to_value(&task).unwrap();
        assert_eq!(json["id"], "JJC-1");
        assert_eq!(json["state"], "Doing");
        assert!(json.get("flow_log").is_some());
        assert!(json.get("updatedAt").is_some());
    }

    #[test]
    fn archivierte_aufgaben_werden_erkannt() {
        let done = Task {
            state: TaskState::Done,
            ..Task::default()
        };
        assert!(done.is_archived());
        let running = Task {
            state: TaskState::Doing,
            ..Task::default()
        };
        assert!(!running.is_archived());
        let archived = Task {
            archived: true,
            ..Task::default()
        };
        assert!(archived.is_archived());
    }

    #[test]
    fn organisationen_werden_uebersetzt() {
        assert_eq!(org_label("中书省"), "Zhongshu");
        assert_eq!(org_label("工部"), "Bauministerium");
        assert_eq!(org_label("Unbekannt"), "Unbekannt");
    }

    #[test]
    fn zeitstempel_werden_gelesen() {
        let value = "2026-03-01T12:00:00+00:00";
        assert!(parse_timestamp(value).is_some());
        assert!(parse_timestamp("2026-03-01 12:00:00").is_some());
        assert!(parse_timestamp("kein Datum").is_none());
    }
}

