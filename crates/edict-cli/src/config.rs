//! Lokale Konfiguration der CLI.
//!
//! Die CLI liest nur ihre eigene Konfigurationsdatei. Dateien des
//! Datenbestands des Servers bleiben unberührt.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use edict_core::filestore::{atomic_write_json, read_json, FileLock};
use serde::{Deserialize, Serialize};

/// Vorgabe, wenn keine Adresse gefunden wird.
pub const DEFAULT_BASE_URL: &str = "http://127.0.0.1:8080";

/// Umgebungsvariable mit der höchsten Priorität.
pub const ENV_API_URL: &str = "EDICT_API_URL";

/// Umgebungsvariable mit der zweiten Priorität.
pub const ENV_SERVER_URL: &str = "EDICT_SERVER_URL";

/// Inhalt der Konfigurationsdatei.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CliConfig {
    /// Adresse des Edict-Servers.
    #[serde(
        rename = "serverUrl",
        default,
        alias = "apiUrl",
        alias = "server_url",
        skip_serializing_if = "Option::is_none"
    )]
    pub server_url: Option<String>,
}

/// Herkunft der Server-Adresse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UrlSource {
    /// Angabe über die Option `--url`.
    Option,
    /// Umgebungsvariable.
    Environment(&'static str),
    /// Konfigurationsdatei.
    ConfigFile,
    /// Vorgabe.
    Default,
}

impl UrlSource {
    /// Deutscher Text zur Herkunft der Adresse.
    pub fn description(&self) -> String {
        match self {
            UrlSource::Option => "Angabe über --url".to_string(),
            UrlSource::Environment(name) => format!("Umgebungsvariable {name}"),
            UrlSource::ConfigFile => format!("Konfigurationsdatei {}", config_path().display()),
            UrlSource::Default => "Vorgabe".to_string(),
        }
    }
}

/// Liefert die Server-Adresse und ihre Herkunft.
///
/// Die Reihenfolge ist Option, Umgebungsvariable, Konfigurationsdatei und
/// zuletzt die Vorgabe.
pub fn resolve_base_url(option: Option<&str>) -> Result<(String, UrlSource)> {
    if let Some(value) = option.map(str::trim).filter(|value| !value.is_empty()) {
        return Ok((value.to_string(), UrlSource::Option));
    }
    for name in [ENV_API_URL, ENV_SERVER_URL] {
        if let Ok(value) = std::env::var(name) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Ok((trimmed.to_string(), UrlSource::Environment(name)));
            }
        }
    }
    let config = load_config()?;
    if let Some(value) = config.server_url.as_deref().map(str::trim) {
        if !value.is_empty() {
            return Ok((value.to_string(), UrlSource::ConfigFile));
        }
    }
    Ok((DEFAULT_BASE_URL.to_string(), UrlSource::Default))
}

/// Pfad der Konfigurationsdatei.
pub fn config_path() -> PathBuf {
    config_dir().join("edict").join("config.json")
}

/// Pfad der Standarddatei für den Live-Status.
pub fn default_status_path() -> PathBuf {
    data_dir().join("edict").join("status.json")
}

/// Verzeichnis der Konfiguration.
fn config_dir() -> PathBuf {
    if let Some(value) = std::env::var_os("XDG_CONFIG_HOME") {
        if !value.is_empty() {
            return PathBuf::from(value);
        }
    }
    if let Some(home) = home_dir() {
        return home.join(".config");
    }
    PathBuf::from(".config")
}

/// Verzeichnis der lokalen Daten.
fn data_dir() -> PathBuf {
    if let Some(value) = std::env::var_os("XDG_DATA_HOME") {
        if !value.is_empty() {
            return PathBuf::from(value);
        }
    }
    if let Some(home) = home_dir() {
        return home.join(".local").join("share");
    }
    PathBuf::from("data")
}

/// Persönliches Verzeichnis des Nutzers.
fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// Liest die Konfigurationsdatei. Fehlt sie, gilt die Vorgabe.
pub fn load_config() -> Result<CliConfig> {
    let path = config_path();
    let value: Option<CliConfig> = read_json(&path).with_context(|| {
        format!(
            "Die Konfiguration {} konnte nicht gelesen werden.",
            path.display()
        )
    })?;
    Ok(value.unwrap_or_default())
}

/// Schreibt die Konfigurationsdatei atomar.
pub fn save_config(config: &CliConfig) -> Result<()> {
    let path = config_path();
    write_json_with_lock(&path, config)
}

/// Schreibt eine JSON-Datei atomar und hält dabei eine Dateisperre.
pub fn write_json_with_lock<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let lock_path = lock_path_for(path);
    let _lock = FileLock::acquire(&lock_path).with_context(|| {
        format!(
            "Die Sperre für {} konnte nicht gesetzt werden.",
            lock_path.display()
        )
    })?;
    atomic_write_json(path, value).with_context(|| {
        format!(
            "Die Datei {} konnte nicht geschrieben werden.",
            path.display()
        )
    })?;
    Ok(())
}

/// Pfad der Sperrdatei neben einer Datei.
fn lock_path_for(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "daten".to_string());
    path.with_file_name(format!(".{name}.lock"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sperrdatei_teilt_den_verzeichnisbaum() {
        let path = PathBuf::from("/tmp/edict/status.json");
        assert_eq!(
            lock_path_for(&path),
            PathBuf::from("/tmp/edict/.status.json.lock")
        );
    }

    #[test]
    fn herkunft_wird_beschrieben() {
        assert_eq!(UrlSource::Option.description(), "Angabe über --url");
        assert_eq!(
            UrlSource::Environment(ENV_API_URL).description(),
            "Umgebungsvariable EDICT_API_URL"
        );
    }
}
