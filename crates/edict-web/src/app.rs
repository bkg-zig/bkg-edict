//! Gemeinsamer Zustand der Oberflaeche, Live-Verbindung und Aktionen.
//!
//! Die Oberflaeche laedt die Aufgaben ueber `/api/live-status`. Ein WebSocket
//! meldet Aenderungen, damit die Anzeige ohne Wartezeit aktualisiert wird.
//! Verlaesst die Verbindung den Server, fragt ein Rueckfall die Aufgabenliste
//! alle fuenf Sekunden ab.

use chrono::{Duration, Utc};
use dioxus::hooks::{UnboundedReceiver, UnboundedSender};
use dioxus::prelude::*;
use futures_util::future::{select, Either};
use futures_util::{FutureExt, SinkExt, StreamExt};
use gloo_net::websocket::futures::WebSocket;
use gloo_net::websocket::Message;
use gloo_timers::future::TimeoutFuture;
use serde_json::{Map, Value};

use crate::api;
use crate::models::{
    AgentActivityData, AgentConfig, AgentsStatusData, ChangeLogEntry, MorningBrief, NewTask,
    OfficialsData, SchedulerStateData, Task, TaskActivityData, TaskState, TodoItem,
};

/// Der sichtbare Bereich der Oberflaeche.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum View {
    /// Board mit den Aufgaben nach Zustand.
    Board,
    /// Uebersichten zu Agenten, Statistik und Berichten.
    Overview,
    /// Detailansicht einer Aufgabe.
    Detail,
}

/// Zustand der Verbindung zum Server.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// Die WebSocket-Verbindung wird aufgebaut.
    Connecting,
    /// Ereignisse kommen ueber WebSocket an.
    Live,
    /// Die Oberflaeche fragt die Aufgabenliste per HTTP ab.
    Polling,
}

impl ConnectionState {
    /// Deutsche Beschreibung des Verbindungszustands.
    pub fn label(self) -> &'static str {
        match self {
            ConnectionState::Connecting => "Verbindung wird aufgebaut",
            ConnectionState::Live => "Live",
            ConnectionState::Polling => "Abfrage alle 5 Sekunden",
        }
    }
}

/// Umfang der angezeigten Aufgaben.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ArchiveFilter {
    /// Offene und archivierte Aufgaben.
    All,
    /// Nur offene Aufgaben.
    Open,
    /// Nur archivierte Aufgaben.
    Archived,
}

impl ArchiveFilter {
    /// Deutsche Bezeichnung der Auswahl.
    pub fn label(self) -> &'static str {
        match self {
            ArchiveFilter::All => "Alle Aufgaben",
            ArchiveFilter::Open => "Nur offene",
            ArchiveFilter::Archived => "Nur archivierte",
        }
    }

    /// Alle verfuegbaren Auswahlwerte.
    pub const ALL: [ArchiveFilter; 3] = [
        ArchiveFilter::All,
        ArchiveFilter::Open,
        ArchiveFilter::Archived,
    ];
}

/// Reihenfolge der Aufgabenliste.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    /// Nach der letzten Aenderung, die neueste zuerst.
    Updated,
    /// Nach dem Titel, alphabetisch.
    Title,
}

impl SortOrder {
    /// Deutsche Bezeichnung der Auswahl.
    pub fn label(self) -> &'static str {
        match self {
            SortOrder::Updated => "Zuletzt aktualisiert",
            SortOrder::Title => "Titel A bis Z",
        }
    }

    /// Alle verfuegbaren Auswahlwerte.
    pub const ALL: [SortOrder; 2] = [SortOrder::Updated, SortOrder::Title];
}

/// Filter der Aufgabenliste.
#[derive(Clone, PartialEq)]
pub struct Filter {
    /// Zustand, nach dem gefiltert wird.
    pub state: Option<TaskState>,
    /// Organisation, nach der gefiltert wird.
    pub org: String,
    /// Text, der im Titel gesucht wird.
    pub text: String,
    /// Umfang der angezeigten Aufgaben.
    pub archive: ArchiveFilter,
    /// Reihenfolge der Aufgabenliste.
    pub sort: SortOrder,
}

impl Default for Filter {
    fn default() -> Self {
        Self {
            state: None,
            org: String::new(),
            text: String::new(),
            archive: ArchiveFilter::All,
            sort: SortOrder::Updated,
        }
    }
}

/// Art einer Meldung am unteren Rand.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    /// Die Aktion wurde ausgefuehrt.
    Success,
    /// Die Aktion ist fehlgeschlagen.
    Error,
    /// Hinweis fuer die Bedienung.
    Info,
}

/// Meldung am unteren Rand der Oberflaeche.
#[derive(Clone, PartialEq)]
pub struct Toast {
    /// Eindeutige Nummer der Meldung.
    pub id: u64,
    /// Text der Meldung.
    pub text: String,
    /// Art der Meldung.
    pub kind: ToastKind,
}

/// Befehl, den die Oberflaeche an die Hintergrundaufgaben schickt.
#[derive(Clone, PartialEq)]
pub enum Command {
    /// Aufgabenliste neu laden.
    ReloadTasks,
    /// Alle Uebersichten neu laden.
    LoadOverview,
    /// Aktivitaet eines Agenten laden.
    LoadAgentActivity(String),
    /// Aktivitaet und Scheduler-Zustand einer Aufgabe laden.
    LoadDetail(String),
    /// Eine Aktion ausfuehren.
    Action(Action),
}

/// Schreibende Aktion der Oberflaeche.
#[derive(Clone, PartialEq)]
pub enum Action {
    /// Zustand der Aufgabe weiterschalten.
    AdvanceState {
        /// Kennung der Aufgabe.
        task_id: String,
        /// Kommentar zum Uebergang.
        comment: String,
    },
    /// Aufgabe archivieren oder wiederherstellen.
    Archive {
        /// Kennung der Aufgabe.
        task_id: String,
        /// Zustand der Archivierung.
        archived: bool,
    },
    /// Alle abgeschlossenen Aufgaben archivieren.
    ArchiveAllDone,
    /// Teilaufgaben speichern.
    UpdateTodos {
        /// Kennung der Aufgabe.
        task_id: String,
        /// Neue Liste der Teilaufgaben.
        todos: Vec<TodoItem>,
    },
    /// Lauf einer Aufgabe steuern.
    TaskControl {
        /// Kennung der Aufgabe.
        task_id: String,
        /// Aktion, etwa `stop`, `cancel` oder `resume`.
        action: String,
        /// Begruendung der Aktion.
        reason: String,
    },
    /// Abnahme einer Aufgabe.
    Review {
        /// Kennung der Aufgabe.
        task_id: String,
        /// Aktion, etwa `approve` oder `reject`.
        action: String,
        /// Kommentar zur Abnahme.
        comment: String,
    },
    /// Agenten aufwecken.
    Wake {
        /// Kennung des Agenten.
        agent_id: String,
        /// Nachricht an den Agenten.
        message: String,
    },
    /// Neue Aufgabe anlegen.
    CreateTask(NewTask),
    /// Modell eines Agenten wechseln.
    SetModel {
        /// Kennung des Agenten.
        agent_id: String,
        /// Kennung des neuen Modells.
        model: String,
    },
}

/// Alle Signale der Oberflaeche.
#[derive(Clone, Copy)]
pub struct AppState {
    /// Aufgabenliste.
    pub tasks: Signal<Vec<Task>>,
    /// Liest die Oberflaeche gerade Aufgaben.
    pub loading: Signal<bool>,
    /// Letzter Fehler beim Laden.
    pub error: Signal<Option<String>>,
    /// Zustand der Verbindung zum Server.
    pub connection: Signal<ConnectionState>,
    /// Meldungen am unteren Rand.
    pub toasts: Signal<Vec<Toast>>,
    /// Zaehler fuer die Nummerierung der Meldungen.
    pub toast_seq: Signal<u64>,
    /// Sichtbarer Bereich.
    pub view: Signal<View>,
    /// Kennung der Aufgabe in der Detailansicht.
    pub selected_task: Signal<String>,
    /// Filter der Aufgabenliste.
    pub filter: Signal<Filter>,
    /// Aktivitaet der gezeigten Aufgabe.
    pub detail_activity: Signal<Option<TaskActivityData>>,
    /// Scheduler-Zustand der gezeigten Aufgabe.
    pub detail_scheduler: Signal<Option<SchedulerStateData>>,
    /// Status der Agenten.
    pub agents_status: Signal<Option<AgentsStatusData>>,
    /// Aktivitaet des gewaehlten Agenten.
    pub agent_activity: Signal<Option<AgentActivityData>>,
    /// Kennung des Agenten in der Aktivitaetsanzeige.
    pub selected_agent: Signal<String>,
    /// Amtliche Statistik.
    pub officials: Signal<Option<OfficialsData>>,
    /// Morgenbericht.
    pub morning_brief: Signal<Option<MorningBrief>>,
    /// Konfiguration der Agenten.
    pub agent_config: Signal<Option<AgentConfig>>,
    /// Protokoll der Modellwechsel.
    pub model_log: Signal<Vec<ChangeLogEntry>>,
    /// Ergebnis des letzten Modellwechsels.
    pub last_result: Signal<Option<Map<String, Value>>>,
    /// Merkt, dass die Aufgabenliste veraltet ist.
    pub dirty: Signal<bool>,
    /// Dialog zum Anlegen einer Aufgabe ist offen.
    pub create_task_open: Signal<bool>,
}

/// Kontext der Oberflaeche mit Zustand und Befehlskanal.
#[derive(Clone)]
pub struct UiContext {
    /// Gemeinsame Signale.
    pub state: AppState,
    /// Kanal zu den Hintergrundaufgaben.
    pub commands: UnboundedSender<Command>,
}

/// Ereignis aus dem WebSocket.
enum WsEvent {
    /// Nachricht des Servers.
    Message(Message),
    /// Verbindung wurde beendet.
    Closed,
    /// Zeit ohne Nachricht verstrichen.
    Timeout,
}

/// Setzt ein Signal und gibt die Warnung des Compilers nicht zu Wort.
fn set_signal<T: PartialEq + 'static>(signal: &mut Signal<T>, value: T) {
    signal.set(value);
}

/// Legt eine Meldung an und beschneidet die Liste auf vier Eintraege.
pub fn push_toast(mut state: AppState, kind: ToastKind, text: impl Into<String>) {
    let id = state.toast_seq.read().wrapping_add(1);
    set_signal(&mut state.toast_seq, id);
    let mut toasts = state.toasts.write();
    toasts.push(Toast {
        id,
        kind,
        text: text.into(),
    });
    if toasts.len() > 4 {
        toasts.remove(0);
    }
}

/// Laedt die Aufgabenliste und schreibt sie in den Zustand.
async fn load_tasks(mut state: AppState) {
    match api::live_status().await {
        Ok(status) => {
            set_signal(&mut state.tasks, status.tasks);
            set_signal(&mut state.loading, false);
            set_signal(&mut state.error, None);
        }
        Err(error) => {
            set_signal(&mut state.loading, false);
            set_signal(&mut state.error, Some(error.message().to_string()));
        }
    }
}

/// Laedt alle Uebersichten.
async fn load_overview(mut state: AppState) {
    match api::agents_status().await {
        Ok(data) => set_signal(&mut state.agents_status, Some(data)),
        Err(error) => push_toast(state, ToastKind::Error, error.message().to_string()),
    }
    match api::officials_stats().await {
        Ok(data) => set_signal(&mut state.officials, Some(data)),
        Err(error) => push_toast(state, ToastKind::Error, error.message().to_string()),
    }
    match api::morning_brief().await {
        Ok(data) => set_signal(&mut state.morning_brief, Some(data)),
        Err(error) => push_toast(state, ToastKind::Error, error.message().to_string()),
    }
    match api::agent_config().await {
        Ok(data) => set_signal(&mut state.agent_config, Some(data)),
        Err(error) => push_toast(state, ToastKind::Error, error.message().to_string()),
    }
    match api::model_change_log().await {
        Ok(entries) => set_signal(&mut state.model_log, entries),
        Err(error) => push_toast(state, ToastKind::Error, error.message().to_string()),
    }
    match api::last_result().await {
        Ok(data) => set_signal(&mut state.last_result, Some(data)),
        Err(error) => push_toast(state, ToastKind::Error, error.message().to_string()),
    }
}

/// Laedt die Aktivitaet eines Agenten.
async fn load_agent_activity(mut state: AppState, agent_id: &str) {
    set_signal(&mut state.selected_agent, agent_id.to_string());
    match api::agent_activity(agent_id).await {
        Ok(data) => set_signal(&mut state.agent_activity, Some(data)),
        Err(error) => push_toast(state, ToastKind::Error, error.message().to_string()),
    }
}

/// Laedt Aktivitaet und Scheduler-Zustand einer Aufgabe.
async fn load_detail(mut state: AppState, task_id: &str) {
    let (activity, scheduler) =
        futures_util::future::join(api::task_activity(task_id), api::scheduler_state(task_id))
            .await;
    match activity {
        Ok(data) => set_signal(&mut state.detail_activity, Some(data)),
        Err(error) => push_toast(state, ToastKind::Error, error.message().to_string()),
    }
    match scheduler {
        Ok(data) => set_signal(&mut state.detail_scheduler, Some(data)),
        Err(error) => push_toast(state, ToastKind::Error, error.message().to_string()),
    }
}

/// Fuehrt eine Aktion aus und meldet das Ergebnis.
async fn run_action(mut state: AppState, action: Action) {
    let outcome = match &action {
        Action::AdvanceState { task_id, comment } => api::advance_state(task_id, comment)
            .await
            .map(|_| "Der Zustand der Aufgabe wurde weitergeschaltet.".to_string()),
        Action::Archive { task_id, archived } => {
            api::archive_task(task_id, *archived).await.map(|_| {
                if *archived {
                    "Die Aufgabe wurde archiviert.".to_string()
                } else {
                    "Die Aufgabe wurde wiederhergestellt.".to_string()
                }
            })
        }
        Action::ArchiveAllDone => api::archive_all_done()
            .await
            .map(|_| "Alle abgeschlossenen Aufgaben wurden archiviert.".to_string()),
        Action::UpdateTodos { task_id, todos } => api::update_todos(task_id, todos)
            .await
            .map(|_| "Die Teilaufgaben wurden gespeichert.".to_string()),
        Action::TaskControl {
            task_id,
            action,
            reason,
        } => api::task_action(task_id, action, reason)
            .await
            .map(|_| "Der Ablauf der Aufgabe wurde gesteuert.".to_string()),
        Action::Review {
            task_id,
            action,
            comment,
        } => api::review_action(task_id, action, comment)
            .await
            .map(|_| "Die Abnahme wurde gespeichert.".to_string()),
        Action::Wake { agent_id, message } => api::agent_wake(agent_id, message)
            .await
            .map(|_| "Der Agent wurde aufgeweckt.".to_string()),
        Action::CreateTask(task) => api::create_task(task)
            .await
            .map(|_| "Die Aufgabe wurde angelegt.".to_string()),
        Action::SetModel { agent_id, model } => api::set_model(agent_id, model)
            .await
            .map(|_| "Das Modell des Agenten wurde gewechselt.".to_string()),
    };

    match outcome {
        Ok(text) => {
            push_toast(state, ToastKind::Success, text);
            set_signal(&mut state.dirty, true);
            load_tasks(state).await;
        }
        Err(error) => push_toast(state, ToastKind::Error, error.message().to_string()),
    }
}

/// Nimmt Befehle entgegen und fuehrt sie aus.
async fn command_loop(mut rx: UnboundedReceiver<Command>, state: AppState) {
    while let Some(command) = rx.next().await {
        match command {
            Command::ReloadTasks => load_tasks(state).await,
            Command::LoadOverview => load_overview(state).await,
            Command::LoadAgentActivity(agent_id) => load_agent_activity(state, &agent_id).await,
            Command::LoadDetail(task_id) => load_detail(state, &task_id).await,
            Command::Action(action) => run_action(state, action).await,
        }
    }
}

/// Laedt die Aufgabenliste regelmaessig und reagiert auf WebSocket-Ereignisse.
async fn refresh_loop(mut state: AppState) {
    load_tasks(state).await;
    let mut last_poll = Utc::now() - Duration::seconds(10);
    loop {
        TimeoutFuture::new(1000).await;
        let dirty = *state.dirty.read();
        let live = *state.connection.read() == ConnectionState::Live;
        if dirty {
            set_signal(&mut state.dirty, false);
            load_tasks(state).await;
            last_poll = Utc::now();
        } else if !live {
            let now = Utc::now();
            if (now - last_poll).num_seconds() >= 5 {
                load_tasks(state).await;
                last_poll = now;
            }
        }
    }
}

/// Haelt die WebSocket-Verbindung offen und meldet Aenderungen.
async fn websocket_loop(mut state: AppState) {
    let mut announced = false;
    loop {
        let url = match api::websocket_url() {
            Ok(url) => url,
            Err(_) => {
                set_signal(&mut state.connection, ConnectionState::Polling);
                TimeoutFuture::new(3000).await;
                continue;
            }
        };

        set_signal(&mut state.connection, ConnectionState::Connecting);
        if let Ok(socket) = WebSocket::open(&url) {
            let (mut sink, mut stream) = socket.split();
            let subscribe = Message::Text(api::subscribe_message(&api::SUBSCRIBED_TOPICS));
            if sink.send(subscribe).await.is_ok() {
                set_signal(&mut state.connection, ConnectionState::Live);
                announced = false;
                loop {
                    let incoming = stream.next().map(|item| match item {
                        Some(Ok(message)) => WsEvent::Message(message),
                        _ => WsEvent::Closed,
                    });
                    let timeout = TimeoutFuture::new(20_000).map(|_| WsEvent::Timeout);
                    let event = match select(Box::pin(incoming), Box::pin(timeout)).await {
                        Either::Left((event, _)) => event,
                        Either::Right((event, _)) => event,
                    };
                    match event {
                        WsEvent::Message(Message::Text(text)) => {
                            if let Some(api::ServerMessage::Event { .. }) =
                                api::parse_server_message(&text)
                            {
                                set_signal(&mut state.dirty, true);
                            }
                        }
                        WsEvent::Message(Message::Bytes(_)) => {}
                        WsEvent::Timeout => {
                            let _ = sink.send(Message::Text(api::ping_message())).await;
                        }
                        WsEvent::Closed => break,
                    }
                }
            }
        }

        set_signal(&mut state.connection, ConnectionState::Polling);
        if !announced {
            announced = true;
            push_toast(
                state,
                ToastKind::Info,
                "Die Live-Verbindung steht nicht. Die Aufgabenliste wird alle 5 Sekunden aktualisiert.",
            );
        }
        TimeoutFuture::new(3000).await;
    }
}

/// Kopfzeile der Oberflaeche mit Navigation und Verbindungsanzeige.
fn header(state: AppState) -> Element {
    let mut view = state.view;
    let mut create_open = state.create_task_open;
    let current = *view.read();

    rsx! {
        header { class: "app-header",
            div { class: "brand",
                h1 { "Edict 2.0" },
                span { class: "brand-subtitle", "Aufgabenverwaltung" }
            }
            nav { class: "main-nav", aria_label: "Hauptnavigation",
                button {
                    class: if current == View::Board { "nav-button nav-button-active" } else { "nav-button" },
                    aria_current: if current == View::Board { "page" } else { "false" },
                    onclick: move |_| view.set(View::Board),
                    "Board"
                }
                button {
                    class: if current == View::Overview { "nav-button nav-button-active" } else { "nav-button" },
                    aria_current: if current == View::Overview { "page" } else { "false" },
                    onclick: move |_| view.set(View::Overview),
                    "Uebersichten"
                }
            }
            div { class: "header-actions",
                span {
                    class: "connection-badge",
                    role: "status",
                    aria_live: "polite",
                    {state.connection.read().label()}
                }
                button {
                    class: "primary-button",
                    onclick: move |_| create_open.set(true),
                    "Neue Aufgabe"
                }
            }
        }
    }
}

/// Die Oberflaeche von Edict.
#[component]
pub fn App() -> Element {
    let state = AppState {
        tasks: use_signal(Vec::new),
        loading: use_signal(|| true),
        error: use_signal(|| None),
        connection: use_signal(|| ConnectionState::Connecting),
        toasts: use_signal(Vec::new),
        toast_seq: use_signal(|| 0),
        view: use_signal(|| View::Board),
        selected_task: use_signal(String::new),
        filter: use_signal(Filter::default),
        detail_activity: use_signal(|| None),
        detail_scheduler: use_signal(|| None),
        agents_status: use_signal(|| None),
        agent_activity: use_signal(|| None),
        selected_agent: use_signal(String::new),
        officials: use_signal(|| None),
        morning_brief: use_signal(|| None),
        agent_config: use_signal(|| None),
        model_log: use_signal(Vec::new),
        last_result: use_signal(|| None),
        dirty: use_signal(|| false),
        create_task_open: use_signal(|| false),
    };

    let command_handle = use_coroutine(move |rx: UnboundedReceiver<Command>| {
        let state = state;
        async move {
            futures_util::future::join(
                command_loop(rx, state),
                futures_util::future::join(refresh_loop(state), websocket_loop(state)),
            )
            .await;
        }
    });

    let commands = command_handle.tx();
    let context = UiContext { state, commands };
    use_context_provider(|| context.clone());

    let current = *state.view.read();

    rsx! {
        style { {crate::styles::STYLES} },
        div { class: "app-shell",
            {header(state)}
            main { class: "app-main",
                match current {
                    View::Board => rsx! { crate::views::board::Board {} },
                    View::Overview => rsx! { crate::views::overview::Overview {} },
                    View::Detail => rsx! { crate::views::task_detail::TaskDetail {} },
                }
            }
            crate::components::toasts::ToastStack {}
            crate::components::task_form::CreateTaskDialog {}
        }
    }
}
