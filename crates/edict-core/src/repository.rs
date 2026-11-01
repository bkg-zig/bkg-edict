//! Ports zwischen fachlichem Kern, Persistenz und Ereignisbus.
//!
//! Die Server-, Worker- und CLI-Crates arbeiten ausschließlich gegen diese
//! Traits. Dadurch bleibt der fachliche Kern frei von Datenbank- und
//! Netzwerkabhängigkeiten und alle Komponenten bleiben austauschbar.

use async_trait::async_trait;
use serde_json::{Map, Value};

use crate::error::AppResult;
use crate::model::{Event, LiveStatus, Task, TaskState, TodoItem};

/// Filter für eine Aufgabenliste.
#[derive(Debug, Clone, Default)]
pub struct TaskFilter {
    pub state: Option<TaskState>,
    pub org: Option<String>,
    pub assignee_org: Option<String>,
    pub priority: Option<String>,
    pub archived: Option<bool>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// Persistenz für Aufgaben.
#[async_trait]
pub trait TaskRepository: Send + Sync {
    /// Legt eine Aufgabe an und gibt sie mit gesetzter Kennung zurück.
    async fn create_task(&self, task: Task) -> AppResult<Task>;

    /// Liest eine Aufgabe über ihre Kennung.
    async fn get_task(&self, id: &str) -> AppResult<Option<Task>>;

    /// Listet Aufgaben nach Filter.
    async fn list_tasks(&self, filter: &TaskFilter) -> AppResult<Vec<Task>>;

    /// Zählt Aufgaben je Zustand.
    async fn count_by_state(&self) -> AppResult<Vec<(String, i64)>>;

    /// Speichert eine geänderte Aufgabe.
    ///
    /// Die Aufgabe trägt die Version, die sie beim Lesen gesehen hat. Passt
    /// die Version in der Datenbank nicht mehr, wurde die Aufgabe zwischenzeitlich
    /// geändert und der Schreibvorgang wird abgelehnt.
    async fn save_task(&self, task: &mut Task) -> AppResult<()>;

    /// Setzt den Archivstatus einer Aufgabe.
    ///
    /// Der Rückgabewert sagt, ob sich der Status geändert hat.
    async fn set_archived(&self, id: &str, archived: bool) -> AppResult<bool>;

    /// Sucht eine Aufgabe über eine Legacy-Kennung wie `JJC-20260301-001`.
    async fn find_by_legacy_id(&self, legacy_id: &str) -> AppResult<Option<Task>>;

    /// Wechselt den Zustand einer Aufgabe und schreibt den Übergang mit.
    ///
    /// Aufgabe, Ereignis und Outbox-Eintrag entstehen in einer Transaktion.
    /// Dadurch kann ein Zustandswechsel nie ohne Ereignis gespeichert werden.
    async fn transition_state(
        &self,
        id: &str,
        new_state: TaskState,
        agent: &str,
        reason: &str,
    ) -> AppResult<Task>;

    /// Hängt einen Fortschrittseintrag an eine Aufgabe an.
    async fn set_progress(
        &self,
        id: &str,
        agent: &str,
        content: &str,
        agent_label: Option<&str>,
    ) -> AppResult<Task>;

    /// Setzt die Teilaufgaben einer Aufgabe.
    async fn set_todos(&self, id: &str, todos: Vec<TodoItem>) -> AppResult<Task>;

    /// Setzt die Scheduler-Angaben einer Aufgabe.
    async fn set_scheduler(&self, id: &str, scheduler: Map<String, Value>) -> AppResult<Task>;

    /// Fordert die Ausführung einer Aufgabe durch einen Agenten an.
    ///
    /// Der Auftrag wird als Ereignis abgelegt. Die Zustellung an das Gateway
    /// übernimmt der Worker.
    async fn request_dispatch(&self, id: &str, agent: &str, message: &str) -> AppResult<Task>;

    /// Liefert die Aufgabenliste mit Synchronisierungsstatus.
    async fn live_status(&self) -> AppResult<LiveStatus>;
}

/// Persistenz für Ereignisse.
#[async_trait]
pub trait EventRepository: Send + Sync {
    /// Speichert ein Ereignis in der Datenbank.
    async fn append_event(&self, event: &Event) -> AppResult<()>;

    /// Liest Ereignisse nach Filtern, newest first.
    async fn list_events(
        &self,
        trace_id: Option<&str>,
        topic: Option<&str>,
        producer: Option<&str>,
        limit: i64,
    ) -> AppResult<Vec<Event>>;
}

/// Veröffentlichung von Ereignissen für Worker und WebSocket.
#[async_trait]
pub trait EventPublisher: Send + Sync {
    /// Veröffentlicht ein Ereignis.
    ///
    /// Das Ereignis wird unverändert weitergereicht. Kennung und Zeitstempel
    /// bleiben damit über Datenbank und Ereignisbus hinweg gleich.
    async fn publish_event(&self, event: &Event) -> AppResult<String>;

    /// Meldet, ob der Bus erreichbar ist.
    async fn is_healthy(&self) -> bool;
}

/// Zähler für Aufgaben je Zustand, gruppiert nach Organisation.
#[async_trait]
pub trait StatisticsRepository: Send + Sync {
    /// Zählt die Aufgaben je Zustand und Organisation.
    async fn count_by_state_and_org(&self) -> AppResult<Vec<(String, String, i64)>>;
}

/// Beanspruchte Ausführung eines Auftrags.
///
/// Der Idempotenzschlüssel `dispatch_id` setzt sich aus Aufgabe, Anforderung
/// und Versuch zusammen. Er geht als `EDICT_DISPATCH_ID` an das Gateway, damit
/// eine wiederholte Ausführung am Gateway erkennbar bleibt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchClaim {
    pub task_id: String,
    pub request_id: String,
    pub attempt: i64,
    pub dispatch_id: String,
    pub owner: String,
}

/// Ergebnis einer Beanspruchung.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaimOutcome {
    /// Der Auftrag wurde für diesen Worker beansprucht.
    Claimed(DispatchClaim),
    /// Ein anderer Worker hält die Sperre. Der Auftrag bleibt unbestätigt.
    Busy,
    /// Der Auftrag wurde bereits erfolgreich ausgeführt.
    Finished,
}

/// Persistenz für die Ausführung von Aufträgen durch das Gateway.
///
/// Die Sperre verhindert, dass ein Auftrag gleichzeitig mehrfach läuft. Läuft
/// der Worker länger als die Lease, darf ein anderer Worker den Auftrag
/// übernehmen.
#[async_trait]
pub trait DispatchRepository: Send + Sync {
    /// Beansprucht einen Auftrag für die Ausführung.
    async fn claim_dispatch(
        &self,
        task_id: &str,
        request_id: &str,
        owner: &str,
        lease_sec: i64,
    ) -> AppResult<ClaimOutcome>;

    /// Meldet das Ende einer Ausführung.
    ///
    /// Bei Erfolg gilt der Auftrag als erledigt und wird nicht erneut
    /// ausgeführt. Bei einem Fehler wird die Sperre freigegeben, damit ein
    /// weiterer Versuch möglich ist.
    async fn finish_dispatch(
        &self,
        claim: &DispatchClaim,
        success: bool,
        error: Option<&str>,
    ) -> AppResult<()>;

    /// Schreibt ein Ereignis und den zugehörigen Outbox-Eintrag.
    ///
    /// Beide entstehen in derselben Transaktion, damit kein Ereignis ohne
    /// Zustellung bleibt.
    async fn append_event_with_outbox(&self, event: &Event) -> AppResult<()>;
}
