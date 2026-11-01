//! Begleitwerkzeuge des Edict-Ablaufs.
//!
//! Die Werkzeuge halten den Rückstand, die Arbeitsbereiche der Agenten, die
//! Prüfungen und die Nachrichten zwischen den Agenten. Das Plugin kennt die
//! Namen und die Zwecke der Werkzeuge und prüft, ob sie auf dem Pfad liegen.
//!
//! Die Liste entspricht der Werkzeugliste des alten Ablaufs. Die Prüfung
//! läuft ohne Shell, damit kein Name in einen Befehl wandert.

use std::path::{Path, PathBuf};

/// Beschreibung eines Begleitwerkzeugs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Werkzeug {
    /// Name des Werkzeugs in der Konfiguration.
    pub name: &'static str,
    /// Programm, das der Ablauf startet.
    pub binary: &'static str,
    /// Zweck des Werkzeugs.
    pub zweck: &'static str,
}

/// Die Werkzeuge, die der Ablauf kennt.
pub const WERKZEUGE: [Werkzeug; 5] = [
    Werkzeug {
        name: "bones",
        binary: "bn",
        zweck: "hält den Rückstand und den Zustand jeder Aufgabe",
    },
    Werkzeug {
        name: "maw",
        binary: "maw",
        zweck: "hält die Arbeitsbereiche der Agenten",
    },
    Werkzeug {
        name: "seal",
        binary: "seal",
        zweck: "hält die Prüfungen und ihre Freigaben",
    },
    Werkzeug {
        name: "rite",
        binary: "rite",
        zweck: "trägt die Nachrichten zwischen den Agenten",
    },
    Werkzeug {
        name: "vessel",
        binary: "vessel",
        zweck: "startet die Agenten in Containern",
    },
];

/// Zustand eines Werkzeugs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Werkzeugstatus {
    /// Name des Werkzeugs.
    pub name: String,
    /// Programm, das der Ablauf startet.
    pub binary: String,
    /// Ob das Programm auf dem Pfad liegt.
    pub vorhanden: bool,
    /// Pfad des Programms, wenn es gefunden wurde.
    pub pfad: Option<PathBuf>,
}

/// Prüft alle Werkzeuge.
///
/// Der Pfad kommt aus `PATH` des Prozesses. Fehlt die Variable, wird kein
/// Werkzeug als vorhanden gemeldet.
#[must_use]
pub fn pruefe_alle() -> Vec<Werkzeugstatus> {
    WERKZEUGE.iter().map(pruefe).collect()
}

/// Prüft ein Werkzeug.
#[must_use]
pub fn pruefe(werkzeug: &Werkzeug) -> Werkzeugstatus {
    let pfad = finde_im_pfad(werkzeug.binary);
    Werkzeugstatus {
        name: werkzeug.name.to_string(),
        binary: werkzeug.binary.to_string(),
        vorhanden: pfad.is_some(),
        pfad,
    }
}

/// Sucht ein Programm in den Verzeichnissen des Pfades.
#[must_use]
pub fn finde_im_pfad(binary: &str) -> Option<PathBuf> {
    if binary.contains('/') {
        let direkt = PathBuf::from(binary);
        return direkt.is_file().then_some(direkt);
    }
    let pfad = std::env::var_os("PATH")?;
    for verzeichnis in std::env::split_paths(&pfad) {
        let kandidat = verzeichnis.join(binary);
        if ist_ausfuehrbar(&kandidat) {
            return Some(kandidat);
        }
    }
    None
}

/// Meldet, ob eine Datei ausführbar ist.
fn ist_ausfuehrbar(pfad: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(pfad)
            .map(|angaben| angaben.is_file() && angaben.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        pfad.is_file()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alle_werkzeuge_haben_einen_zweck() {
        for werkzeug in WERKZEUGE {
            assert!(!werkzeug.name.is_empty());
            assert!(!werkzeug.binary.is_empty());
            assert!(!werkzeug.zweck.is_empty());
        }
    }

    #[test]
    fn vorhandenes_programm_wird_gefunden() {
        let gefunden = finde_im_pfad("sh");
        assert!(gefunden.is_some(), "sh liegt auf dem Pfad");
    }

    #[test]
    fn unbekanntes_programm_wird_nicht_gefunden() {
        assert!(finde_im_pfad("edict-programm-das-es-nicht-gibt").is_none());
    }

    #[test]
    fn status_nennt_name_und_programm() {
        let status = pruefe(&WERKZEUGE[0]);
        assert_eq!(status.name, "bones");
        assert_eq!(status.binary, "bn");
    }
}
