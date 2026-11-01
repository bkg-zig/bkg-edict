//! Aufruf des Gateways ZeptoClaw.
//!
//! Der Aufruf lautet `zeptoclaw agent --template <rolle> --no-stream -m
//! <nachricht>`. ZeptoClaw kennt keinen Schalter für eine Rolle, sie kommt
//! über die Vorlage und über die `SOUL.md` im Arbeitsverzeichnis.

use std::fmt;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use edict_core::config::Settings;
use tokio::process::Command;

/// Vorlagen, die ZeptoClaw selbst mitbringt.
const BUILTIN_TEMPLATES: [&str; 5] = ["coder", "researcher", "writer", "assistant", "task_manager"];

/// Höchste Länge der übernommenen Standardausgabe.
const MAX_STDOUT: usize = 5000;

/// Höchste Länge der übernommenen Fehlerausgabe.
const MAX_STDERR: usize = 2000;

/// Anfrage an das Gateway.
#[derive(Debug, Clone)]
pub struct GatewayRequest {
    /// Rolle des Agenten, zugleich Name der Vorlage.
    pub role: String,
    /// Nachricht an den Agenten.
    pub message: String,
    /// Technische Kennung der Aufgabe.
    pub task_id: String,
    /// Fachliche Kennung der Aufgabe.
    pub legacy_id: String,
    /// Kennung der Ereigniskette.
    pub trace_id: String,
    /// Idempotenzschlüssel des Versuchs.
    pub dispatch_id: String,
}

/// Ergebnis eines Gateway-Aufrufs.
#[derive(Debug, Clone)]
pub struct GatewayOutcome {
    /// Rückgabewert des Prozesses.
    pub return_code: i64,
    /// Ausgabe auf der Standardausgabe.
    pub stdout: String,
    /// Ausgabe auf der Fehlerausgabe.
    pub stderr: String,
}

/// Fehler eines Gateway-Aufrufs.
#[derive(Debug)]
pub enum GatewayError {
    /// Die Binary des Gateways wurde nicht gefunden.
    BinaryMissing(String),
    /// Die Vorlage der Rolle existiert nicht.
    TemplateMissing(String),
    /// Der Aufruf hat die Zeitgrenze überschritten.
    Timeout,
    /// Der Prozess ließ sich nicht starten.
    StartFailed(String),
}

impl fmt::Display for GatewayError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GatewayError::BinaryMissing(path) => {
                write!(formatter, "Gateway {path} wurde nicht gefunden")
            }
            GatewayError::TemplateMissing(role) => {
                write!(formatter, "Vorlage {role} wurde nicht gefunden")
            }
            GatewayError::Timeout => write!(formatter, "Zeitüberschreitung beim Aufruf"),
            GatewayError::StartFailed(reason) => {
                write!(formatter, "Gateway nicht gestartet: {reason}")
            }
        }
    }
}

/// Führt einen Aufruf des Gateways aus.
pub async fn run(
    settings: &Settings,
    request: GatewayRequest,
) -> Result<GatewayOutcome, GatewayError> {
    if !template_exists(&request.role) {
        return Err(GatewayError::TemplateMissing(request.role));
    }

    let workspace = role_workspace(settings, &request.role);
    let mut command = Command::new(&settings.zeptoclaw_bin);
    command
        .arg("agent")
        .arg("--template")
        .arg(&request.role)
        .arg("--no-stream")
        .arg("-m")
        .arg(&request.message)
        .current_dir(working_directory(settings))
        .env("ZEPTOCLAW_AGENTS_DEFAULTS_WORKSPACE", &workspace)
        .env("EDICT_TASK_ID", &request.task_id)
        .env("EDICT_LEGACY_ID", &request.legacy_id)
        .env("EDICT_TRACE_ID", &request.trace_id)
        .env("EDICT_DISPATCH_ID", &request.dispatch_id)
        .env("EDICT_API_URL", api_url(settings))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let timeout = Duration::from_secs(settings.dispatch_timeout_sec.max(1));
    let output = match tokio::time::timeout(timeout, command.output()).await {
        Ok(Ok(output)) => output,
        Ok(Err(error)) => {
            let message = error.to_string();
            if error.kind() == std::io::ErrorKind::NotFound {
                return Err(GatewayError::BinaryMissing(settings.zeptoclaw_bin.clone()));
            }
            return Err(GatewayError::StartFailed(message));
        }
        Err(_) => return Err(GatewayError::Timeout),
    };

    let stdout = tail(&output.stdout, MAX_STDOUT);
    let stderr = tail(&output.stderr, MAX_STDERR);
    let return_code = output.status.code().map(i64::from).unwrap_or(-1);

    if return_code != 0 && stderr.contains("not found. Available templates") {
        return Err(GatewayError::TemplateMissing(request.role));
    }

    Ok(GatewayOutcome {
        return_code,
        stdout,
        stderr,
    })
}

/// Prüft, ob eine Vorlage für die Rolle vorhanden ist.
///
/// ZeptoClaw bringt einige Vorlagen mit. Eigene Vorlagen liegen unter
/// `~/.zeptoclaw/templates` als JSON- oder TOML-Datei.
pub fn template_exists(role: &str) -> bool {
    if BUILTIN_TEMPLATES.contains(&role) {
        return true;
    }
    let directory = config_directory().join("templates");
    directory.join(format!("{role}.json")).is_file()
        || directory.join(format!("{role}.toml")).is_file()
}

/// Verzeichnis der Konfiguration von ZeptoClaw.
fn config_directory() -> PathBuf {
    match std::env::var("HOME") {
        Ok(home) if !home.trim().is_empty() => PathBuf::from(home).join(".zeptoclaw"),
        _ => PathBuf::from(".zeptoclaw"),
    }
}

/// Arbeitsverzeichnis des Aufrufs.
fn working_directory(settings: &Settings) -> PathBuf {
    match &settings.zeptoclaw_project_dir {
        Some(directory) if !directory.trim().is_empty() => PathBuf::from(directory),
        _ => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    }
}

/// Arbeitsverzeichnis der Rolle.
///
/// ZeptoClaw liest die `SOUL.md` aus dem Arbeitsverzeichnis. Liegt unter dem
/// Projektverzeichnis ein Ordner der Rolle, wird er verwendet. Sonst gilt das
/// Projektverzeichnis selbst.
fn role_workspace(settings: &Settings, role: &str) -> PathBuf {
    let base = working_directory(settings);
    let candidate = base.join("agents").join(role);
    if candidate.is_dir() {
        candidate
    } else {
        base
    }
}

/// Adresse der API für die Agenten.
fn api_url(settings: &Settings) -> String {
    if let Ok(value) = std::env::var("EDICT_API_URL") {
        if !value.trim().is_empty() {
            return value.trim().to_string();
        }
    }
    let host = match settings.backend_host.as_str() {
        "0.0.0.0" | "" | "::" => "127.0.0.1",
        other => other,
    };
    format!("http://{host}:{}", settings.backend_port)
}

/// Kürzt eine Ausgabe auf die letzten Zeichen.
fn tail(raw: &[u8], max: usize) -> String {
    let text = String::from_utf8_lossy(raw);
    if text.len() <= max {
        return text.to_string();
    }
    let start = text.len() - max;
    let start = text
        .char_indices()
        .find(|(index, _)| *index >= start)
        .map(|(index, _)| index)
        .unwrap_or(0);
    text[start..].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ausgabe_wird_auf_die_ende_beschaenkt() {
        let raw = vec![b'a'; 10];
        assert_eq!(tail(&raw, 5), "aaaaa");
    }

    #[test]
    fn eingebaute_vorlagen_gibts_ohne_datei() {
        assert!(template_exists("coder"));
        assert!(!template_exists("rolle_ohne_vorlage"));
    }
}
