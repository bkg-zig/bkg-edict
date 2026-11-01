//! Fehler des fachlichen Kerns.
//!
//! Der Kern kennt keine Datenbank- oder Netzwerktreiber. Konkrete Treiberfehler
//! wandern in den aufrufenden Crates in diese Fehlerart um.

use thiserror::Error;

/// Fehler, die über API, CLI und Worker gemeinsam auftreten können.
#[derive(Debug, Error)]
pub enum AppError {
    #[error("Aufgabe {0} wurde nicht gefunden")]
    TaskNotFound(String),

    #[error("Ungültiger Zustandswechsel: {from} -> {to}. Erlaubt: {allowed:?}")]
    InvalidTransition {
        from: String,
        to: String,
        allowed: Vec<String>,
    },

    #[error("Ungültiger Aufgabenzustand: {0}")]
    InvalidState(String),

    #[error("Eingabe ungültig: {0}")]
    Validation(String),

    #[error("Konflikt: {0}")]
    Conflict(String),

    #[error("Datenbankfehler: {0}")]
    Database(String),

    #[error("Redis-Fehler: {0}")]
    Redis(String),

    #[error("Dateisystemfehler: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON-Fehler: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Konfigurationsfehler: {0}")]
    Config(String),

    #[error("Externe Anfrage fehlgeschlagen: {0}")]
    Http(String),

    #[error("{0}")]
    Internal(String),
}

impl AppError {
    /// Kurzer, maschinenlesbarer Fehlercode für API-Antworten.
    pub fn code(&self) -> &'static str {
        match self {
            AppError::TaskNotFound(_) => "task_not_found",
            AppError::InvalidTransition { .. } => "invalid_transition",
            AppError::InvalidState(_) => "invalid_state",
            AppError::Validation(_) => "validation_error",
            AppError::Conflict(_) => "conflict",
            AppError::Database(_) => "database_error",
            AppError::Redis(_) => "redis_error",
            AppError::Io(_) => "io_error",
            AppError::Json(_) => "json_error",
            AppError::Config(_) => "config_error",
            AppError::Http(_) => "http_error",
            AppError::Internal(_) => "internal_error",
        }
    }

    /// HTTP-Statuscode, der zu diesem Fehler passt.
    pub fn status(&self) -> u16 {
        match self {
            AppError::TaskNotFound(_) => 404,
            AppError::InvalidTransition { .. } => 400,
            AppError::InvalidState(_) => 400,
            AppError::Validation(_) => 400,
            AppError::Conflict(_) => 409,
            AppError::Config(_) => 500,
            _ => 500,
        }
    }
}

/// Ergebnis des fachlichen Kerns.
pub type AppResult<T> = Result<T, AppError>;
