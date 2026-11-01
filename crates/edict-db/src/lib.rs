//! Persistenz und Ereignisbus des Edict-Port.
//!
//! Das Crate bündelt die PostgreSQL-Anbindung, die Umsetzung der Ports aus
//! `edict-core` und das Relay, das Ereignisse aus der Outbox in Redis
//! veröffentlicht.

pub mod database;
pub mod dispatch;
pub mod events;
pub mod outbox;
pub mod redis_bus;
pub mod task_store;

pub use edict_core::repository::{
    ClaimOutcome, DispatchClaim, DispatchRepository, EventPublisher, EventRepository,
    StatisticsRepository, TaskFilter, TaskRepository,
};

pub use database::Database;
pub use dispatch::DispatchStore;
pub use outbox::OutboxRelay;
pub use redis_bus::RedisBus;
pub use task_store::TaskStore;
