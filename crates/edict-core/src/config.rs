//! Konfiguration aus Umgebungsvariablen.
//!
//! Die Datenbank- und Redis-Variablen entsprechen dem bisherigen Betrieb.
//! Das Gateway ist ZeptoClaw. Die Variablen `ZEPTOCLAW_BIN`,
//! `ZEPTOCLAW_HEALTH_URL` und `ZEPTOCLAW_PROJECT_DIR` steuern den Aufruf des
//! Gateways. Werte aus `.env` werden beim Start einmalig geladen.

use crate::error::{AppError, AppResult};

/// Laufzeitkonfiguration des Systems.
#[derive(Debug, Clone)]
pub struct Settings {
    pub postgres_host: String,
    pub postgres_port: u16,
    pub postgres_db: String,
    pub postgres_user: String,
    pub postgres_password: String,
    pub database_url: String,
    pub redis_url: String,
    pub backend_host: String,
    pub backend_port: u16,
    pub secret_key: String,
    pub debug: bool,
    pub zeptoclaw_bin: String,
    pub zeptoclaw_health_url: String,
    pub zeptoclaw_project_dir: Option<String>,
    pub legacy_data_dir: String,
    pub legacy_tasks_file: String,
    pub stall_threshold_sec: i64,
    pub max_dispatch_retry: i64,
    pub allowed_origins: Vec<String>,
    pub admin_token: Option<String>,
    pub dispatch_timeout_sec: u64,
    pub max_concurrent_dispatch: usize,
}

impl Settings {
    /// Lädt die Konfiguration aus der Umgebung.
    pub fn from_env() -> AppResult<Self> {
        let postgres_host = env_or("POSTGRES_HOST", "localhost");
        let postgres_port = env_parse("POSTGRES_PORT", 5432u16)?;
        let postgres_db = env_or("POSTGRES_DB", "edict");
        let postgres_user = env_or("POSTGRES_USER", "edict");
        let postgres_password = env_or("POSTGRES_PASSWORD", "edict_secret_change_me");

        let database_url = match std::env::var("DATABASE_URL") {
            Ok(value) if !value.trim().is_empty() => normalize_database_url(&value),
            _ => format!(
                "postgres://{postgres_user}:{postgres_password}@{postgres_host}:{postgres_port}/{postgres_db}"
            ),
        };

        let allowed_origins = std::env::var("EDICT_ALLOWED_ORIGINS")
            .ok()
            .map(|value| {
                value
                    .split(',')
                    .map(|item| item.trim().to_string())
                    .filter(|item| !item.is_empty())
                    .collect()
            })
            .unwrap_or_else(|| vec!["http://localhost:7891".to_string()]);

        Ok(Settings {
            postgres_host,
            postgres_port,
            postgres_db,
            postgres_user,
            postgres_password,
            database_url,
            redis_url: env_or("REDIS_URL", "redis://localhost:6379/0"),
            backend_host: env_or("BACKEND_HOST", "0.0.0.0"),
            backend_port: env_parse("BACKEND_PORT", 8000u16)?,
            secret_key: env_or("SECRET_KEY", "change-me-in-production"),
            debug: env_parse("DEBUG", false)?,
            zeptoclaw_bin: env_or("ZEPTOCLAW_BIN", "zeptoclaw"),
            zeptoclaw_health_url: env_or("ZEPTOCLAW_HEALTH_URL", "http://127.0.0.1:9090/health"),
            zeptoclaw_project_dir: std::env::var("ZEPTOCLAW_PROJECT_DIR")
                .ok()
                .filter(|value| !value.trim().is_empty()),
            legacy_data_dir: env_or("LEGACY_DATA_DIR", "../data"),
            legacy_tasks_file: env_or("LEGACY_TASKS_FILE", "../data/tasks_source.json"),
            stall_threshold_sec: env_parse("STALL_THRESHOLD_SEC", 180i64)?,
            max_dispatch_retry: env_parse("MAX_DISPATCH_RETRY", 3i64)?,
            allowed_origins,
            admin_token: std::env::var("EDICT_ADMIN_TOKEN")
                .ok()
                .filter(|value| !value.trim().is_empty()),
            dispatch_timeout_sec: env_parse("EDICT_DISPATCH_TIMEOUT_SEC", 300u64)?,
            max_concurrent_dispatch: env_parse("EDICT_MAX_CONCURRENT_DISPATCH", 3usize)?,
        })
    }

    /// Lädt die Konfiguration und beendet den Prozess bei einem Fehler.
    pub fn load_or_exit() -> Self {
        match Settings::from_env() {
            Ok(settings) => settings,
            Err(error) => {
                eprintln!("Konfiguration konnte nicht geladen werden: {error}");
                std::process::exit(1);
            }
        }
    }
}

/// Wandelt eine SQLAlchemy-URL in eine PostgreSQL-URL für SQLx um.
pub fn normalize_database_url(value: &str) -> String {
    let trimmed = value.trim();
    if let Some(rest) = trimmed.strip_prefix("postgresql+asyncpg://") {
        format!("postgresql://{rest}")
    } else if let Some(rest) = trimmed.strip_prefix("postgres+asyncpg://") {
        format!("postgres://{rest}")
    } else {
        trimmed.to_string()
    }
}

fn env_or(name: &str, fallback: &str) -> String {
    match std::env::var(name) {
        Ok(value) if !value.trim().is_empty() => value,
        _ => fallback.to_string(),
    }
}

fn env_parse<T>(name: &str, fallback: T) -> AppResult<T>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    match std::env::var(name) {
        Ok(value) if !value.trim().is_empty() => value
            .trim()
            .parse::<T>()
            .map_err(|error| AppError::Config(format!("{name}: {error}"))),
        _ => Ok(fallback),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqlalchemy_url_wird_umgesetzt() {
        assert_eq!(
            normalize_database_url("postgresql+asyncpg://a:b@host/db"),
            "postgresql://a:b@host/db"
        );
        assert_eq!(
            normalize_database_url("postgresql://a:b@host/db"),
            "postgresql://a:b@host/db"
        );
    }
}
