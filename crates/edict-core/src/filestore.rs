//! Atomarer Dateizugriff für Konfigurations- und Exportdateien.
//!
//! Der fachliche Kern bleibt frei von Datenbank- und Netzwerktreibern.
//! Das Lesen und Schreiben kleiner JSON-Dateien unter einer festen Freigabe
//! gehört jedoch zum gemeinsamen Vertrag von Server, Worker und CLI.
//! Deshalb liegt die Implementierung hier und nicht in drei Kopien.

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use fs2::FileExt;
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::error::{AppError, AppResult};

/// Exklusive Sperre auf eine Datei oder ein Verzeichnis.
///
/// Die Sperre wird beim Fallen des Werts automatisch freigegeben.
pub struct FileLock {
    file: File,
    path: PathBuf,
}

impl FileLock {
    /// Nimmt eine exklusive Sperre auf dem angegebenen Pfad.
    pub fn acquire(path: &Path) -> AppResult<Self> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(path)?;
        file.lock_exclusive()?;
        Ok(FileLock {
            file,
            path: path.to_path_buf(),
        })
    }

    /// Pfad der gesperrten Datei.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

/// Liest eine JSON-Datei. Fehlt die Datei, ist das Ergebnis `None`.
pub fn read_json<T: DeserializeOwned>(path: &Path) -> AppResult<Option<T>> {
    match std::fs::read(path) {
        Ok(bytes) => {
            if bytes.iter().all(|byte| byte.is_ascii_whitespace()) {
                return Ok(None);
            }
            let value = serde_json::from_slice(&bytes)?;
            Ok(Some(value))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(AppError::Io(error)),
    }
}

/// Schreibt eine JSON-Datei atomar.
///
/// Die Datei wird zuerst in eine temporäre Datei im selben Verzeichnis
/// geschrieben und danach mit `rename` ersetzt. Ein Absturz während des
/// Schreibens hinterlässt deshalb keine halbfertige Datei.
pub fn atomic_write_json<T: Serialize>(path: &Path, value: &T) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let serialized = serde_json::to_vec_pretty(value)?;
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "daten".to_string());
    let temporary = path.with_file_name(format!(".{file_name}.tmp"));

    {
        let mut file = File::create(&temporary)?;
        file.write_all(&serialized)?;
        file.write_all(b"\n")?;
        file.flush()?;
        file.sync_all()?;
    }
    std::fs::rename(&temporary, path)?;
    Ok(())
}

/// Liest eine Textdatei. Fehlt die Datei, ist das Ergebnis `None`.
pub fn read_text(path: &Path) -> AppResult<Option<String>> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(AppError::Io(error)),
    }
}

/// Schreibt eine Textdatei atomar.
pub fn atomic_write_text(path: &Path, text: &str) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "daten".to_string());
    let temporary = path.with_file_name(format!(".{file_name}.tmp"));
    {
        let mut file = File::create(&temporary)?;
        file.write_all(text.as_bytes())?;
        file.flush()?;
        file.sync_all()?;
    }
    std::fs::rename(&temporary, path)?;
    Ok(())
}

/// Liest eine Datei zeilenweise.
pub fn read_lines(path: &Path) -> AppResult<Vec<String>> {
    let mut content = String::new();
    match File::open(path) {
        Ok(mut file) => {
            file.read_to_string(&mut content)?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(AppError::Io(error)),
    }
    Ok(content.lines().map(|line| line.to_string()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_wird_atomar_geschrieben_und_gelesen() {
        let dir = std::env::temp_dir().join(format!("edict-filestore-{}", uuid::Uuid::new_v4()));
        let path = dir.join("test.json");
        atomic_write_json(&path, &serde_json::json!({"a": 1})).unwrap();
        let value: serde_json::Value = read_json(&path).unwrap().unwrap();
        assert_eq!(value["a"], 1);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn fehlende_datei_liefert_none() {
        let path = std::env::temp_dir().join("edict-filestore-fehlt.json");
        std::fs::remove_file(&path).ok();
        let value: Option<serde_json::Value> = read_json(&path).unwrap();
        assert!(value.is_none());
    }
}
