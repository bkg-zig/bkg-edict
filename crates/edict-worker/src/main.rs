//! Worker des Edict-Port.
//!
//! Der Worker hat zwei Aufgaben. Er stellt Ereignisse aus der Outbox in
//! Redis zu und führt Ausführungsaufträge über das Gateway ZeptoClaw aus.

mod dispatcher;
mod gateway;

use std::sync::Arc;
use std::time::Duration;

use edict_core::config::Settings;
use edict_core::repository::EventPublisher;
use edict_db::{Database, DispatchStore, OutboxRelay, RedisBus, TaskStore};
use tracing_subscriber::EnvFilter;

use crate::dispatcher::Dispatcher;

/// Abstand zwischen zwei Durchläufen des Outbox-Relay.
const RELAY_INTERVAL: Duration = Duration::from_secs(1);

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let settings = Settings::load_or_exit();

    let database = Database::connect(&settings.database_url).await?;
    database.migrate().await?;
    let pool = database.pool().clone();

    let bus = Arc::new(RedisBus::new(&settings.redis_url)?);

    let relay = OutboxRelay::new(pool.clone(), bus.clone() as Arc<dyn EventPublisher>);
    let relay_task = tokio::spawn(async move {
        if let Err(error) = relay.run(RELAY_INTERVAL).await {
            tracing::error!(%error, "Relay beendet");
        }
    });

    let dispatcher = Dispatcher::new(
        bus,
        DispatchStore::new(pool.clone()),
        TaskStore::new(pool),
        settings,
    );
    let dispatcher_task = tokio::spawn(async move {
        if let Err(error) = dispatcher.run().await {
            tracing::error!(%error, "Ausführungsworker beendet");
        }
    });

    tokio::select! {
        _ = relay_task => tracing::warn!("Relay beendet"),
        _ = dispatcher_task => tracing::warn!("Ausführungsworker beendet"),
        _ = tokio::signal::ctrl_c() => tracing::info!("Worker wird beendet"),
    }
    Ok(())
}
