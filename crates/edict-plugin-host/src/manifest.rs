//! Manifest eines Plugins.
//!
//! Das Manifest liegt als `plugin.json` im Pluginverzeichnis. Es nennt Name,
//! Version, Protokollversion, Fähigkeiten, Ereignisabonnements und das
//! Kommando, mit dem der Host den Kindprozess startet.
//!
//! Beispiel:
//!
//! ```json
//! {
//!   "name": "gitclaw",
//!   "version": "2.0.0",
//!   "protocol_version": 1,
//!   "command": "./edict-plugin-gitclaw",
//!   "capabilities": ["gitclaw.review"],
//!   "subscriptions": ["task.review.request"]
//! }
//! ```

use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use edict_core::{AppError, AppResult};

/// Name der Manifestdatei im Pluginverzeichnis.
pub const MANIFEST_FILE_NAME: &str = "plugin.json";

/// Beschreibung eines Plugins für den Host.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Manifest {
    /// Eindeutiger Name des Plugins. Er ist der Schlüssel in der Registry.
    pub name: String,
    /// Version des Plugins nach SemVer.
    pub version: String,
    /// Protokollversion, die der Host erwartet.
    pub protocol_version: u32,
    /// Kommando zum Start des Kindprozesses, relativ zum Pluginverzeichnis.
    pub command: String,
    /// Fähigkeiten, die der Host bei diesem Plugin aufrufen darf.
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// Ereignisarten, die das Plugin empfangen möchte.
    #[serde(default)]
    pub subscriptions: Vec<String>,
    /// Kurze Beschreibung für Listen und Protokolle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl Manifest {
    /// Liest ein Manifest aus JSON-Text.
    pub fn parse(text: &str) -> AppResult<Self> {
        let manifest: Manifest = serde_json::from_str(text)
            .map_err(|error| AppError::Config(format!("Manifest unlesbar: {error}")))?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// Liest das Manifest aus einem Pluginverzeichnis.
    pub fn load(plugin_dir: &Path) -> AppResult<Self> {
        let path = plugin_dir.join(MANIFEST_FILE_NAME);
        let text = std::fs::read_to_string(&path).map_err(|error| {
            AppError::Config(format!("Manifest {} nicht lesbar: {error}", path.display()))
        })?;
        Self::parse(&text)
    }

    /// Prüft die Pflichtangaben des Manifests.
    pub fn validate(&self) -> AppResult<()> {
        if self.name.is_empty()
            || !self.name.chars().all(|zeichen| {
                zeichen.is_ascii_lowercase() || zeichen.is_ascii_digit() || zeichen == '-'
            })
        {
            return Err(AppError::Config(format!(
                "Ungültiger Pluginname: {}",
                self.name
            )));
        }
        if self.version.trim().is_empty() {
            return Err(AppError::Config(format!(
                "Plugin {} nennt keine Version",
                self.name
            )));
        }
        if self.command.trim().is_empty() {
            return Err(AppError::Config(format!(
                "Plugin {} nennt kein Kommando",
                self.name
            )));
        }
        Ok(())
    }

    /// Löst das Kommando im Pluginverzeichnis auf.
    ///
    /// Das Kommando ist ein relativer Pfad. Absolute Pfade und Pfade, die aus
    /// dem Pluginverzeichnis hinausführen, werden abgewiesen. Der Host startet
    /// das Kommando ohne Shell.
    pub fn resolve_command(&self, plugin_dir: &Path) -> AppResult<PathBuf> {
        let raw = self.command.trim();
        let relative = Path::new(raw);
        if relative.is_absolute() {
            return Err(AppError::Config(format!(
                "Kommando von Plugin {} muss relativ sein: {raw}",
                self.name
            )));
        }
        if relative
            .components()
            .any(|teil| matches!(teil, Component::ParentDir | Component::RootDir))
        {
            return Err(AppError::Config(format!(
                "Kommando von Plugin {} verlässt das Pluginverzeichnis: {raw}",
                self.name
            )));
        }

        let basis = plugin_dir.canonicalize().map_err(|error| {
            AppError::Config(format!(
                "Pluginverzeichnis {} nicht lesbar: {error}",
                plugin_dir.display()
            ))
        })?;
        let kandidat = basis.join(relative);
        let aufgeloest = kandidat.canonicalize().map_err(|error| {
            AppError::Config(format!(
                "Kommando {} nicht gefunden: {error}",
                kandidat.display()
            ))
        })?;
        if !aufgeloest.starts_with(&basis) {
            return Err(AppError::Config(format!(
                "Kommando {} liegt außerhalb des Pluginverzeichnisses",
                aufgeloest.display()
            )));
        }
        if !aufgeloest.is_file() {
            return Err(AppError::Config(format!(
                "Kommando {} ist keine Datei",
                aufgeloest.display()
            )));
        }
        Ok(aufgeloest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BEISPIEL: &str = r#"{
        "name": "gitclaw",
        "version": "2.0.0",
        "protocol_version": 1,
        "command": "./edict-plugin-gitclaw",
        "capabilities": ["gitclaw.review"],
        "subscriptions": ["task.review.request"]
    }"#;

    #[test]
    fn manifest_wird_gelesen() {
        let manifest = Manifest::parse(BEISPIEL).unwrap();
        assert_eq!(manifest.name, "gitclaw");
        assert_eq!(manifest.protocol_version, 1);
        assert_eq!(manifest.capabilities, vec!["gitclaw.review"]);
        assert_eq!(manifest.subscriptions, vec!["task.review.request"]);
    }

    #[test]
    fn kaputtes_json_wird_abgewiesen() {
        let fehler = Manifest::parse("{ name: ").unwrap_err();
        assert!(matches!(fehler, AppError::Config(_)));
    }

    #[test]
    fn ungueltiger_name_wird_abgewiesen() {
        let text = BEISPIEL.replace("\"gitclaw\"", "\"GitClaw!\"");
        let fehler = Manifest::parse(&text).unwrap_err();
        assert!(matches!(fehler, AppError::Config(_)));
    }

    #[test]
    fn absolutes_kommando_wird_abgewiesen() {
        let manifest = Manifest::parse(BEISPIEL).unwrap();
        let mut manifest = manifest;
        manifest.command = "/usr/bin/env".to_string();
        let fehler = manifest.resolve_command(Path::new("/tmp")).unwrap_err();
        assert!(matches!(fehler, AppError::Config(_)));
    }

    #[test]
    fn kommando_ausserhalb_des_verzeichnisses_wird_abgewiesen() {
        let mut manifest = Manifest::parse(BEISPIEL).unwrap();
        manifest.command = "../edict-plugin-gitclaw".to_string();
        let fehler = manifest.resolve_command(Path::new("/tmp")).unwrap_err();
        assert!(matches!(fehler, AppError::Config(_)));
    }

    #[test]
    fn fehlendes_kommando_wird_gemeldet() {
        let manifest = Manifest::parse(BEISPIEL).unwrap();
        let fehler = manifest
            .resolve_command(Path::new("/tmp/gibt-es-nicht-12345"))
            .unwrap_err();
        assert!(matches!(fehler, AppError::Config(_)));
    }
}
