//! Fachlicher Kern von Edict.
//!
//! Enthält die Aufgaben- und Ereignismodelle, die Zustandsmaschine der
//!三省六部-Verwaltung, die Agentenmetadaten und die deutschen Anzeigetexte.
//! Das Modul kennt weder HTTP noch Datenbank und bleibt damit für Server,
//! Worker, CLI und Oberfläche gemeinsam nutzbar.

pub mod agents;
pub mod config;
pub mod error;
pub mod filestore;
pub mod labels;
pub mod model;
pub mod repository;
pub mod service;
pub mod state_machine;

pub use error::{AppError, AppResult};
pub use model::{
    topics, Event, FlowEntry, Heartbeat, LiveStatus, ProgressEntry, SchedulerMeta, Task, TaskState,
    TodoItem,
};
pub use repository::{
    EventPublisher, EventRepository, StatisticsRepository, TaskFilter, TaskRepository,
};
pub use state_machine::{can_transition, STATE_TRANSITIONS, TERMINAL_STATES};
