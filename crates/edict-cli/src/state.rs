//! Übersetzung von Zustandsnamen zwischen Eingabe und Serverwert.

use anyhow::{anyhow, Result};
use edict_core::labels;
use edict_core::model::TaskState;

/// Alle Zustände in der Reihenfolge der Zustandsmaschine.
pub const STATES: [TaskState; 11] = [
    TaskState::Taizi,
    TaskState::Zhongshu,
    TaskState::Menxia,
    TaskState::Assigned,
    TaskState::Next,
    TaskState::Doing,
    TaskState::Review,
    TaskState::Done,
    TaskState::Blocked,
    TaskState::Cancelled,
    TaskState::Pending,
];

/// Wandelt eine Eingabe in einen Zustand um.
///
/// Erlaubt sind die kanonischen Namen wie `Doing` und die deutschen
/// Anzeigenamen wie `In Arbeit`.
pub fn parse_state(input: &str) -> Result<TaskState> {
    let normalized = input.trim();
    if normalized.is_empty() {
        return Err(anyhow!("Der Zustand darf nicht leer sein."));
    }
    if let Ok(state) = normalized.parse::<TaskState>() {
        return Ok(state);
    }
    for state in STATES {
        if labels::state_label(state).eq_ignore_ascii_case(normalized) {
            return Ok(state);
        }
    }
    Err(anyhow!(
        "Unbekannter Zustand „{normalized}“. Erlaubt sind: {}.",
        allowed_states()
    ))
}

/// Liste der erlaubten Zustände für Fehlermeldungen.
pub fn allowed_states() -> String {
    STATES
        .iter()
        .map(|state| state.as_str())
        .collect::<Vec<&str>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kanonische_namen_werden_gelesen() {
        assert_eq!(parse_state("doing").unwrap(), TaskState::Doing);
        assert_eq!(parse_state("Doing").unwrap(), TaskState::Doing);
        assert_eq!(parse_state(" Blocked ").unwrap(), TaskState::Blocked);
    }

    #[test]
    fn deutsche_namen_werden_gelesen() {
        assert_eq!(parse_state("In Arbeit").unwrap(), TaskState::Doing);
        assert_eq!(parse_state("blockiert").unwrap(), TaskState::Blocked);
        assert_eq!(parse_state("Abgeschlossen").unwrap(), TaskState::Done);
        assert_eq!(parse_state("Offen").unwrap(), TaskState::Pending);
    }

    #[test]
    fn unbekannte_namen_werden_abgelehnt() {
        let error = parse_state("Fertig").unwrap_err();
        assert!(error.to_string().contains("Fertig"));
        assert!(error.to_string().contains("Doing"));
        assert!(parse_state("").is_err());
    }

    #[test]
    fn erlaubte_zustaende_werden_aufgezaehlt() {
        let text = allowed_states();
        assert!(text.contains("Taizi"));
        assert!(text.contains("Cancelled"));
    }
}
