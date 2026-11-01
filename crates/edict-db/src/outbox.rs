//! Relay zwischen Outbox und Ereignisbus.
//!
//! Jede schreibende Operation legt das Ereignis und den Outbox-Eintrag in
//! derselben Transaktion an. Das Relay nimmt die Einträge in Reihenfolge der
//! Outbox-Kennung, veröffentlicht sie in Redis und markiert sie danach als
//! zugestellt. Ein Fehler beim Veröffenteln zählt den Versuch und lässt den
//! Eintrag für den nächsten Durchlauf stehen.

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use serde_json::{Map, Value};
use sqlx::postgres::PgPool;
use sqlx::Row;
use uuid::Uuid;

use edict_core::error::{AppError, AppResult};
use edict_core::model::Event;
use edict_core::repository::EventPublisher;

/// Standardzahl der Einträge je Durchlauf.
pub const DEFAULT_BATCH_SIZE: i64 = 100;

/// Zustellt Ereignisse aus der Outbox an den Ereignisbus.
pub struct OutboxRelay {
    pool: PgPool,
    publisher: Arc<dyn EventPublisher>,
}

impl OutboxRelay {
    /// Legt das Relay auf Pool und Ereignisbus an.
    pub fn new(pool: PgPool, publisher: Arc<dyn EventPublisher>) -> Self {
        Self { pool, publisher }
    }

    /// Zählt die Einträge, die noch nicht zugestellt sind.
    pub async fn pending_count(&self) -> AppResult<i64> {
        let row = sqlx::query("SELECT COUNT(*) AS total FROM outbox WHERE published = FALSE")
            .fetch_one(&self.pool)
            .await
            .map_err(|error| AppError::Database(format!("Outbox nicht lesbar: {error}")))?;
        Ok(row.try_get("total").unwrap_or(0))
    }

    /// Stellt einen Stapel zu und liefert die Zahl der zugestellten Ereignisse.
    pub async fn relay_once(&self, limit: i64) -> AppResult<usize> {
        let mut transaction =
            self.pool.begin().await.map_err(|error| {
                AppError::Database(format!("Transaktion fehlgeschlagen: {error}"))
            })?;

        let rows = sqlx::query(
            "SELECT o.id, e.event_id, e.trace_id, e.topic, e.event_type, e.producer, \
             e.payload, e.meta, e.timestamp FROM outbox o JOIN events e \
             ON e.event_id = o.event_id WHERE o.published = FALSE \
             ORDER BY o.id ASC LIMIT $1 FOR UPDATE OF o SKIP LOCKED",
        )
        .bind(limit)
        .fetch_all(&mut *transaction)
        .await
        .map_err(|error| AppError::Database(format!("Outbox nicht lesbar: {error}")))?;

        let mut delivered = 0usize;
        for row in &rows {
            let outbox_id: i64 = row.try_get("id").unwrap_or_default();
            let event = map_event(row)?;
            match self.publisher.publish_event(&event).await {
                Ok(_) => {
                    sqlx::query(
                        "UPDATE outbox SET published = TRUE, published_at = $2, \
                         attempts = attempts + 1, last_error = NULL WHERE id = $1",
                    )
                    .bind(outbox_id)
                    .bind(Utc::now())
                    .execute(&mut *transaction)
                    .await
                    .map_err(|error| {
                        AppError::Database(format!("Outbox nicht aktualisierbar: {error}"))
                    })?;
                    delivered += 1;
                }
                Err(error) => {
                    sqlx::query(
                        "UPDATE outbox SET attempts = attempts + 1, last_error = $2 WHERE id = $1",
                    )
                    .bind(outbox_id)
                    .bind(error.to_string())
                    .execute(&mut *transaction)
                    .await
                    .map_err(|update_error| {
                        AppError::Database(format!("Outbox nicht aktualisierbar: {update_error}"))
                    })?;
                }
            }
        }

        transaction
            .commit()
            .await
            .map_err(|error| AppError::Database(format!("Transaktion fehlgeschlagen: {error}")))?;
        Ok(delivered)
    }

    /// Stellt die Outbox fortlaufend zu.
    ///
    /// Der Aufruf endet nur bei einem Datenbankfehler. Fehler beim Ereignisbus
    /// werden gezählt und im nächsten Durchlauf erneut versucht.
    pub async fn run(&self, interval: Duration) -> AppResult<()> {
        loop {
            match self.relay_once(DEFAULT_BATCH_SIZE).await {
                Ok(count) if count > 0 => {
                    tracing::debug!(count, "Ereignisse zugestellt");
                }
                Ok(_) => {}
                Err(error) => {
                    tracing::warn!(%error, "Zustellung der Outbox fehlgeschlagen");
                }
            }
            tokio::time::sleep(interval).await;
        }
    }
}

/// Wandelt eine Zeile in ein Ereignis.
fn map_event(row: &sqlx::postgres::PgRow) -> AppResult<Event> {
    let event_id: Uuid = row
        .try_get("event_id")
        .map_err(|error| AppError::Database(format!("Kennung unlesbar: {error}")))?;
    let timestamp: chrono::DateTime<Utc> = row
        .try_get("timestamp")
        .map_err(|error| AppError::Database(format!("Zeitstempel unlesbar: {error}")))?;
    let payload: Value = row
        .try_get("payload")
        .map_err(|error| AppError::Database(format!("Nutzlast unlesbar: {error}")))?;
    let meta: Value = row
        .try_get("meta")
        .map_err(|error| AppError::Database(format!("Metadaten unlesbar: {error}")))?;

    Ok(Event {
        event_id: event_id.to_string(),
        trace_id: row.try_get("trace_id").unwrap_or_default(),
        timestamp: timestamp.to_rfc3339(),
        topic: row.try_get("topic").unwrap_or_default(),
        event_type: row.try_get("event_type").unwrap_or_default(),
        producer: row.try_get("producer").unwrap_or_default(),
        payload: match payload {
            Value::Object(map) => map,
            _ => Map::new(),
        },
        meta: match meta {
            Value::Object(map) => map,
            _ => Map::new(),
        },
    })
}
