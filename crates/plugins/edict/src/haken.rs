//! Anleitungen für die Agenten.
//!
//! Der alte Ablauf gab dem Harness zu bestimmten Zeitpunkten eine Anleitung
//! auf die Standardausgabe. Das Plugin bildet diese Anleitungen ab und gibt
//! sie auf Anfrage zurück.
//!
//! Drei Zeitpunkte kennt der Ablauf:
//!
//! | Haken | Zeitpunkt |
//! | ----- | --------- |
//! | `session-start` | Beginn einer Sitzung |
//! | `post-tool-call` | nach einem Werkzeugaufruf |
//! | `session-end` | Ende einer Sitzung |

/// Zeitpunkt, an dem ein Haken feuert.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zeitpunkt {
    /// Beginn einer Sitzung.
    Sitzungsstart,
    /// Nach einem Werkzeugaufruf.
    NachWerkzeugaufruf,
    /// Ende einer Sitzung.
    Sitzungsende,
}

impl Zeitpunkt {
    /// Name des Zeitpunkts, wie er in der Konfiguration der Harness steht.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Zeitpunkt::Sitzungsstart => "SessionStart",
            Zeitpunkt::NachWerkzeugaufruf => "PostToolUse",
            Zeitpunkt::Sitzungsende => "SessionEnd",
        }
    }

    /// Liest den Zeitpunkt aus dem Namen.
    #[must_use]
    pub fn aus_name(name: &str) -> Option<Self> {
        match name.trim() {
            "SessionStart" | "PreCompact" => Some(Zeitpunkt::Sitzungsstart),
            "PostToolUse" => Some(Zeitpunkt::NachWerkzeugaufruf),
            "SessionEnd" => Some(Zeitpunkt::Sitzungsende),
            _ => None,
        }
    }
}

/// Ein Haken des Ablaufs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Haken {
    /// Name des Hakens.
    pub name: &'static str,
    /// Zeitpunkte, an denen der Haken feuert.
    pub zeitpunkte: &'static [Zeitpunkt],
}

/// Die Haken, die der Ablauf kennt.
pub const HAKEN: [Haken; 3] = [
    Haken {
        name: "session-start",
        zeitpunkte: &[Zeitpunkt::Sitzungsstart],
    },
    Haken {
        name: "post-tool-call",
        zeitpunkte: &[Zeitpunkt::NachWerkzeugaufruf],
    },
    Haken {
        name: "session-end",
        zeitpunkte: &[Zeitpunkt::Sitzungsende],
    },
];

/// Findet einen Haken nach Name.
///
/// Ältere Namen bleiben gültig, damit eine Harness aus der Zeit der
/// Botbox-Werkzeuge weiterläuft.
#[must_use]
pub fn finde(name: &str) -> Option<&'static Haken> {
    let name = name.trim();
    HAKEN
        .iter()
        .find(|haken| haken.name == name)
        .or_else(|| match name {
            "pre-compact" | "precompact" => {
                HAKEN.iter().find(|haken| haken.name == "session-start")
            }
            "post-tool-use" => HAKEN.iter().find(|haken| haken.name == "post-tool-call"),
            _ => None,
        })
}

/// Baut die Anleitung eines Hakens.
///
/// Die Anleitung nennt den Ort des Agenten, den Kanal des Projekts und die
/// Werkzeuge, die zur Verfügung stehen. Ohne erkannten Kanal bleibt der
/// Absatz weg.
#[must_use]
pub fn anleitung(haken: &Haken, kanal: Option<&str>, agent: Option<&str>) -> String {
    let mut text = String::new();
    text.push_str(&format!("Haken {haken_name}.", haken_name = haken.name));
    if let Some(agent) = agent {
        text.push_str(&format!(" Du arbeitest als {agent}."));
    }
    if let Some(kanal) = kanal {
        text.push_str(&format!(" Der Kanal des Projekts ist {kanal}."));
    }
    text.push_str(" Halte deine Identität im Kanal besetzt, solange du arbeitest.");
    text
}

/// Meldet, ob ein Agentenname dem Muster des Busses entspricht.
///
/// Das Muster ist `[a-z0-9][a-z0-9-/]*`. Ein Name, der nicht passt, würde
/// einen anderen Agenten ansprechen.
#[must_use]
pub fn agentenname_gueltig(name: &str) -> bool {
    !name.is_empty()
        && name.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-' || byte == b'/'
        })
        && !name.starts_with('-')
        && !name.starts_with('/')
}

/// Liest die Sitzungskennung aus der Nutzlast eines Hakens.
///
/// Eine Nutzlast ohne brauchbare Kennung ist kein Fehler. Die Harness hat
/// dann nichts genannt.
#[must_use]
pub fn sitzungskennung(nutzlast: Option<&str>) -> Option<String> {
    let nutzlast = nutzlast?;
    let wert: serde_json::Value = serde_json::from_str(nutzlast).ok()?;
    let id = wert.get("session_id")?.as_str()?.trim();
    if id.is_empty()
        || id.len() > 256
        || id.starts_with('-')
        || !id.chars().all(|zeichen| zeichen.is_ascii_graphic())
    {
        return None;
    }
    Some(id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn haeken_wurden_gefunden() {
        assert!(finde("session-start").is_some());
        assert!(finde("post-tool-call").is_some());
        assert!(finde("session-end").is_some());
        assert!(finde("gibt-es-nicht").is_none());
    }

    #[test]
    fn alter_name_bleibt_gueltig() {
        assert_eq!(
            finde("post-tool-use").map(|haken| haken.name),
            Some("post-tool-call")
        );
    }

    #[test]
    fn zeitpunkte_lesen_namen() {
        assert_eq!(
            Zeitpunkt::aus_name("SessionStart"),
            Some(Zeitpunkt::Sitzungsstart)
        );
        assert_eq!(
            Zeitpunkt::aus_name("PostToolUse"),
            Some(Zeitpunkt::NachWerkzeugaufruf)
        );
        assert_eq!(
            Zeitpunkt::aus_name("SessionEnd"),
            Some(Zeitpunkt::Sitzungsende)
        );
        assert_eq!(Zeitpunkt::aus_name("Unbekannt"), None);
    }

    #[test]
    fn anleitung_nennt_agent_und_kanal() {
        let haken = finde("session-start").unwrap();
        let text = anleitung(haken, Some("projekt"), Some("zhongshu"));
        assert!(text.contains("session-start"));
        assert!(text.contains("zhongshu"));
        assert!(text.contains("projekt"));
    }

    #[test]
    fn agentennamen_werden_geprueft() {
        assert!(agentenname_gueltig("zhongshu"));
        assert!(agentenname_gueltig("team/agent-1"));
        assert!(!agentenname_gueltig("Agent"));
        assert!(!agentenname_gueltig("-agent"));
        assert!(!agentenname_gueltig(""));
    }

    #[test]
    fn sitzungskennung_kommt_aus_der_nutzlast() {
        assert_eq!(
            sitzungskennung(Some(r#"{"session_id":"abc"}"#)),
            Some("abc".to_string())
        );
        assert_eq!(sitzungskennung(Some("{}")), None);
        assert_eq!(sitzungskennung(None), None);
    }
}
