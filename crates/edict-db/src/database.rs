//! Verbindung zu PostgreSQL und Ausführung der Migrationen.

use std::time::Duration;

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

use edict_core::error::{AppError, AppResult};

/// Verbindungspool für PostgreSQL.
#[derive(Clone)]
pub struct Database {
    pool: PgPool,
}

impl Database {
    /// Baut einen Pool zur angegebenen URL auf.
    pub async fn connect(database_url: &str) -> AppResult<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .min_connections(1)
            .acquire_timeout(Duration::from_secs(10))
            .connect(database_url)
            .await
            .map_err(|error| {
                AppError::Database(format!("Verbindung zu PostgreSQL fehlgeschlagen: {error}"))
            })?;
        Ok(Self { pool })
    }

    /// Führt alle eingebetteten Migrationen aus.
    pub async fn migrate(&self) -> AppResult<()> {
        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await
            .map_err(|error| AppError::Database(format!("Migration fehlgeschlagen: {error}")))
    }

    /// Der zugrunde liegende Pool.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Prüft, ob die Datenbank erreichbar ist.
    pub async fn is_healthy(&self) -> bool {
        sqlx::query("SELECT 1").fetch_one(&self.pool).await.is_ok()
    }
}
