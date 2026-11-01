//! Gemeinsames Schreiben von Ereignissen und Outbox-Einträgen.
//!
//! Ereignis und Outbox-Eintrag entstehen immer in derselben Transaktion.
//! Nur so bleibt die Zustellung an den Ereignisbus mit dem Bestand
//! verknüpft.

use chrono::{DateTime, NaiveDateTime, Utc};
use serde_json::Value;
use sqlx::PgConnection;
use uuid::Uuid;

use edict_core::error::{AppError, AppResult};
use edict_core::model::Event;

/// Schreibt ein Ereignis und den zugehörigen Outbox-Eintrag.
///
/// Der Aufruf erwartet eine offene Transaktion. Der Aufrufer entscheidet, ob
/// die Transaktion danach freigegeben oder verworfen wird.
pub async fn append_event_with_outbox(
    connection: &mut PgConnection,
    event: &Event,
) -> AppResult<()> {
    let event_id = parse_uuid(&event.event_id)?;
    let task_id = event
        .payload
        .get("task_id")
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok());
    let timestamp = parse_timestamp(&event.timestamp)?;

    sqlx::query(
        "INSERT INTO events (event_id, task_id, trace_id, topic, event_type, producer, \
         payload, meta, timestamp) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
    )
    .bind(event_id)
    .bind(task_id)
    .bind(&event.trace_id)
    .bind(&event.topic)
    .bind(&event.event_type)
    .bind(&event.producer)
    .bind(Value::Object(event.payload.clone()))
    .bind(Value::Object(event.meta.clone()))
    .bind(timestamp)
    .execute(&mut *connection)
    .await
    .map_err(|error| AppError::Database(format!("Ereignis nicht gespeichert: {error}")))?;

    sqlx::query("INSERT INTO outbox (event_id) VALUES ($1)")
        .bind(event_id)
        .execute(&mut *connection)
        .await
        .map_err(|error| {
            AppError::Database(format!("Outbox-Eintrag nicht gespeichert: {error}"))
        })?;
    Ok(())
}

/// Wandelt eine Zeichenkette in eine UUID.
pub(crate) fn parse_uuid(value: &str) -> AppResult<Uuid> {
    Uuid::parse_str(value.trim())
        .map_err(|error| AppError::Validation(format!("Ungültige Kennung {value}: {error}")))
}

/// Liest einen Zeitstempel im RFC-Format oder im Format des Altbestands.
pub(crate) fn parse_timestamp(value: &str) -> AppResult<DateTime<Utc>> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(Utc::now());
    }
    if let Ok(parsed) = DateTime::parse_from_rfc3339(trimmed) {
        return Ok(parsed.with_timezone(&Utc));
    }
    if let Ok(parsed) = NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%d %H:%M:%S") {
        return Ok(parsed.and_utc());
    }
    Err(AppError::Validation(format!(
        "Zeitstempel {value} lässt sich nicht lesen"
    )))
}
