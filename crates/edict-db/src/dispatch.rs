//! Sperren für die Ausführung durch das Gateway.
//!
//! Jeder Ausführungsauftrag bekommt eine Sperre in PostgreSQL. Die Sperre
//! nennt den ausführenden Worker, die Laufzeit der Sperre und den Versuch.
//! Läuft der Worker länger als die Sperre, darf ein anderer Worker den
//! Auftrag übernehmen. Ist ein Auftrag erfolgreich gelaufen, wird er nicht
//! erneut ausgeführt.

use chrono::{DateTime, Utc};
use sqlx::postgres::PgPool;
use sqlx::Row;
use uuid::Uuid;

use edict_core::error::{AppError, AppResult};
use edict_core::model::Event;
use edict_core::repository::{ClaimOutcome, DispatchClaim, DispatchRepository};

use crate::events::append_event_with_outbox;

/// Status einer Sperre.
const STATUS_RUNNING: &str = "running";
const STATUS_SUCCEEDED: &str = "succeeded";
const STATUS_FAILED: &str = "failed";

/// Persistenz der Ausführungssperren.
#[derive(Clone)]
pub struct DispatchStore {
    pool: PgPool,
}

impl DispatchStore {
    /// Legt den Datenbestand auf dem Pool an.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Der zugrunde liegende Pool.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

#[async_trait::async_trait]
impl DispatchRepository for DispatchStore {
    async fn claim_dispatch(
        &self,
        task_id: &str,
        request_id: &str,
        owner: &str,
        lease_sec: i64,
    ) -> AppResult<ClaimOutcome> {
        let task_uuid = Uuid::parse_str(task_id.trim()).map_err(|error| {
            AppError::Validation(format!("Ungültige Aufgabenkennung {task_id}: {error}"))
        })?;
        let request_uuid = Uuid::parse_str(request_id.trim()).map_err(|error| {
            AppError::Validation(format!("Ungültige Anforderungskennung {request_id}: {error}"))
        })?;

        let mut transaction = self.pool.begin().await.map_err(|error| {
            AppError::Database(format!("Transaktion fehlgeschlagen: {error}"))
        })?;
        let connection = &mut *transaction;

        let row = sqlx::query(
            "SELECT attempt, status, lease_until FROM dispatch_leases \
             WHERE task_id = $1 AND request_id = $2 FOR UPDATE",
        )
        .bind(task_uuid)
        .bind(request_uuid)
        .fetch_optional(&mut *connection)
        .await
        .map_err(|error| {
            AppError::Database(format!("Ausführungssperre nicht lesbar: {error}"))
        })?;

        let previous_attempt: i64 = row
            .as_ref()
            .map(|row| row.try_get("attempt").unwrap_or(0))
            .unwrap_or(0);
        let previous_status: String = row
            .as_ref()
            .map(|row| row.try_get("status").unwrap_or_default())
            .unwrap_or_default();
        let lease_valid: bool = match row.as_ref() {
            Some(row) => {
                let lease_until: DateTime<Utc> = row.try_get("lease_until").unwrap_or_else(|_| {
                    Utc::now() - chrono::Duration::seconds(1)
                });
                previous_status == STATUS_RUNNING && lease_until > Utc::now()
            }
            None => false,
        };

        if previous_status == STATUS_SUCCEEDED {
            transaction
                .commit()
                .await
                .map_err(|error| AppError::Database(format!("Transaktion fehlgeschlagen: {error}")))?;
            return Ok(ClaimOutcome::Finished);
        }

        if lease_valid {
            transaction
                .commit()
                .await
                .map_err(|error| AppError::Database(format!("Transaktion fehlgeschlagen: {error}")))?;
            return Ok(ClaimOutcome::Busy);
        }

        let attempt = previous_attempt + 1;
        let dispatch_id = format!("{task_uuid}:{request_uuid}:{attempt}");

        sqlx::query(
            "INSERT INTO dispatch_leases (task_id, request_id, attempt, dispatch_id, owner, \
             status, lease_until, updated_at) VALUES ($1, $2, $3, $4, $5, 'running', \
             now() + make_interval(secs => $6::double precision), now()) \
             ON CONFLICT (task_id, request_id) DO UPDATE SET attempt = EXCLUDED.attempt, \
             dispatch_id = EXCLUDED.dispatch_id, owner = EXCLUDED.owner, status = 'running', \
             lease_until = EXCLUDED.lease_until, last_error = NULL, updated_at = now()",
        )
        .bind(task_uuid)
        .bind(request_uuid)
        .bind(attempt)
        .bind(&dispatch_id)
        .bind(owner)
        .bind(lease_sec as f64)
        .execute(&mut *connection)
        .await
        .map_err(|error| {
            AppError::Database(format!("Ausführungssperre nicht gespeichert: {error}"))
        })?;

        transaction
            .commit()
            .await
            .map_err(|error| AppError::Database(format!("Transaktion fehlgeschlagen: {error}")))?;

        Ok(ClaimOutcome::Claimed(DispatchClaim {
            task_id: task_uuid.to_string(),
            request_id: request_uuid.to_string(),
            attempt,
            dispatch_id,
            owner: owner.to_string(),
        }))
    }

    async fn finish_dispatch(
        &self,
        claim: &DispatchClaim,
        success: bool,
        error: Option<&str>,
    ) -> AppResult<()> {
        let task_uuid = Uuid::parse_str(claim.task_id.trim()).map_err(|error| {
            AppError::Validation(format!("Ungültige Aufgabenkennung: {error}"))
        })?;
        let request_uuid = Uuid::parse_str(claim.request_id.trim()).map_err(|error| {
            AppError::Validation(format!("Ungültige Anforderungskennung: {error}"))
        })?;
        let status = if success {
            STATUS_SUCCEEDED
        } else {
            STATUS_FAILED
        };

        sqlx::query(
            "UPDATE dispatch_leases SET status = $4, lease_until = now(), last_error = $5, \
             updated_at = now() WHERE task_id = $1 AND request_id = $2 AND attempt = $3",
        )
        .bind(task_uuid)
        .bind(request_uuid)
        .bind(claim.attempt)
        .bind(status)
        .bind(error)
        .execute(&self.pool)
        .await
        .map_err(|error| {
            AppError::Database(format!("Ausführungssperre nicht aktualisierbar: {error}"))
        })?;
        Ok(())
    }

    async fn append_event_with_outbox(&self, event: &Event) -> AppResult<()> {
        let mut transaction = self.pool.begin().await.map_err(|error| {
            AppError::Database(format!("Transaktion fehlgeschlagen: {error}"))
        })?;
        let connection = &mut *transaction;
        append_event_with_outbox(connection, event).await?;
        transaction
            .commit()
            .await
            .map_err(|error| AppError::Database(format!("Transaktion fehlgeschlagen: {error}")))?;
        Ok(())
    }
}
