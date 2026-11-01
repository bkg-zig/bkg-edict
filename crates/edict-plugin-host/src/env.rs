//! Positivliste der Umgebung des Kindprozesses.
//!
//! Ein Plugin erbt nichts aus der Umgebung des Hosts. Der Host übergibt nur
//! `PATH`, `LANG` und `HOME`. `HOME` zeigt auf das Pluginverzeichnis, damit
//! das Plugin seine eigenen Dateien dort ablegen kann und keine fremden
//! Pfade aus dem Benutzerkonto sieht.

use std::collections::BTreeMap;
use std::path::Path;

/// Namen der Variablen, die der Host an ein Plugin übergibt.
pub const ALLOWED_ENV_VARS: [&str; 3] = ["PATH", "LANG", "HOME"];

/// Variablen, die niemals an ein Plugin gelangen dürfen.
///
/// Die Positivliste lässt sie ohnehin nicht zu. Die Liste dient der
/// Dokumentation und der Prüfung in Tests.
pub const FORBIDDEN_ENV_VARS: [&str; 6] = [
    "EDICT_ADMIN_TOKEN",
    "DATABASE_URL",
    "REDIS_URL",
    "SECRET_KEY",
    "POSTGRES_PASSWORD",
    "EDICT_SECRET_KEY",
];

/// Standardwert für `PATH`, falls der Host keinen setzt.
pub const DEFAULT_PATH: &str = "/usr/local/bin:/usr/bin:/bin";

/// Standardwert für `LANG`, falls der Host keinen setzt.
pub const DEFAULT_LANG: &str = "C.UTF-8";

/// Baut die Umgebung des Kindprozesses.
///
/// `host_env` enthält die Umgebung des Hosts. Daraus werden nur `PATH` und
/// `LANG` übernommen. `HOME` zeigt auf das Pluginverzeichnis.
pub fn build_plugin_env(
    plugin_dir: &Path,
    host_env: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    let mut env = BTreeMap::new();
    let path = host_env
        .get("PATH")
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_PATH);
    env.insert("PATH".to_string(), path.to_string());
    let lang = host_env
        .get("LANG")
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_LANG);
    env.insert("LANG".to_string(), lang.to_string());
    env.insert(
        "HOME".to_string(),
        plugin_dir.to_string_lossy().to_string(),
    );
    env
}

/// Liest die Umgebung des Hosts in eine Map.
pub fn host_environment() -> BTreeMap<String, String> {
    std::env::vars().collect()
}

/// Prüft, ob eine Variable als Geheimnis gilt.
pub fn is_forbidden(name: &str) -> bool {
    FORBIDDEN_ENV_VARS
        .iter()
        .any(|verboten| verboten.eq_ignore_ascii_case(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect()
    }

    #[test]
    fn geheimnisse_werden_nicht_uebergeben() {
        let host = map(&[
            ("PATH", "/usr/bin"),
            ("LANG", "de_DE.UTF-8"),
            ("EDICT_ADMIN_TOKEN", "super-geheim"),
            ("DATABASE_URL", "postgres://localhost/edict"),
            ("REDIS_URL", "redis://localhost:6379"),
            ("SECRET_KEY", "change-me"),
        ]);
        let plugin_dir = Path::new("/tmp/plugins/gitclaw");
        let env = build_plugin_env(plugin_dir, &host);

        assert_eq!(env.len(), 3);
        assert_eq!(env.get("PATH").map(String::as_str), Some("/usr/bin"));
        assert_eq!(env.get("LANG").map(String::as_str), Some("de_DE.UTF-8"));
        assert_eq!(
            env.get("HOME").map(String::as_str),
            Some("/tmp/plugins/gitclaw")
        );
        for verboten in FORBIDDEN_ENV_VARS {
            assert!(!env.contains_key(verboten), "{verboten} wurde übergeben");
        }
    }

    #[test]
    fn fehlende_werte_erhalten_standards() {
        let env = build_plugin_env(Path::new("/tmp/plugin"), &BTreeMap::new());
        assert_eq!(env.get("PATH").map(String::as_str), Some(DEFAULT_PATH));
        assert_eq!(env.get("LANG").map(String::as_str), Some(DEFAULT_LANG));
    }

    #[test]
    fn erlaubte_namen_sind_genau_drei() {
        assert_eq!(ALLOWED_ENV_VARS.len(), 3);
        assert!(ALLOWED_ENV_VARS.contains(&"PATH"));
        assert!(ALLOWED_ENV_VARS.contains(&"LANG"));
        assert!(ALLOWED_ENV_VARS.contains(&"HOME"));
    }
}
