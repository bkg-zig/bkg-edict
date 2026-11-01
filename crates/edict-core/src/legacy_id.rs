//! Erzeugung der fachlichen Legacy-Kennung.
//!
//! Der Altbestand kennzeichnet Aufgaben nach dem Muster `JJC-JJJJMMTT-NNN`.
//! Die laufende Nummer beginnt an jedem Tag bei 1 und zählt die Kennungen
//! desselben Tages hoch. Diese Funktion rechnet nur und kennt weder Datenbank
//! noch Dateisystem. Der Aufrufer liefert die bereits vergebenen Kennungen.

use chrono::NaiveDate;

/// Präfix der fachlichen Kennung.
pub const LEGACY_ID_PREFIX: &str = "JJC";

/// Erzeugt die nächste fachliche Kennung für den angegebenen Tag.
///
/// `existing` enthält alle bereits vergebenen Kennungen. Der Tagsteuerung
/// folgen nur die Kennungen, die zum selben Tag gehören. Die Nummer ist
/// dreistellig mit führenden Nullen.
pub fn next_legacy_id(day: NaiveDate, existing: &[String]) -> String {
    let day_text = day.format("%Y%m%d").to_string();
    let prefix = format!("{LEGACY_ID_PREFIX}-{day_text}-");

    let highest = existing
        .iter()
        .filter_map(|id| id.strip_prefix(&prefix))
        .filter_map(|suffix| suffix.parse::<u32>().ok())
        .max();

    match highest {
        Some(value) => format!("{prefix}{:03}", value + 1),
        None => format!("{prefix}001"),
    }
}

/// Prüft, ob eine Zeichenkette dem Muster der fachlichen Kennung entspricht.
pub fn is_legacy_id(value: &str) -> bool {
    let mut parts = value.split('-');
    let prefix = parts.next();
    let day = parts.next();
    let number = parts.next();
    if parts.next().is_some() {
        return false;
    }
    match (prefix, day, number) {
        (Some(prefix), Some(day), Some(number)) => {
            prefix.eq_ignore_ascii_case(LEGACY_ID_PREFIX)
                && day.len() == 8
                && day.chars().all(|character| character.is_ascii_digit())
                && !number.is_empty()
                && number.chars().all(|character| character.is_ascii_digit())
        }
        _ => false,
    }
}

/// Liefert den Tag aus einer fachlichen Kennung.
///
/// Kennungen ohne gültiges Datum liefern keinen Wert.
pub fn legacy_id_day(value: &str) -> Option<NaiveDate> {
    let mut parts = value.split('-');
    let prefix = parts.next()?;
    let day = parts.next()?;
    if !prefix.eq_ignore_ascii_case(LEGACY_ID_PREFIX) {
        return None;
    }
    NaiveDate::parse_from_str(day, "%Y%m%d").ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tag() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, 1).unwrap()
    }

    #[test]
    fn erste_kennung_des_tages_ist_eins() {
        assert_eq!(next_legacy_id(tag(), &[]), "JJC-20261101-001");
    }

    #[test]
    fn nummer_laeuft_je_tag_weiter() {
        let vorhanden = vec![
            "JJC-20261101-001".to_string(),
            "JJC-20261101-002".to_string(),
            "JJC-20261031-007".to_string(),
        ];
        assert_eq!(next_legacy_id(tag(), &vorhanden), "JJC-20261101-003");
    }

    #[test]
    fn kennung_other_tage_zaehlen_nicht() {
        let vorhanden = vec!["JJC-20261031-099".to_string()];
        assert_eq!(next_legacy_id(tag(), &vorhanden), "JJC-20261101-001");
    }

    #[test]
    fn ungueltige_suffixe_werden_ignoriert() {
        let vorhanden = vec!["JJC-20261101-abc".to_string()];
        assert_eq!(next_legacy_id(tag(), &vorhanden), "JJC-20261101-001");
    }

    #[test]
    fn muster_wird_geprueft() {
        assert!(is_legacy_id("JJC-20261101-001"));
        assert!(is_legacy_id("jjc-20261101-001"));
        assert!(!is_legacy_id("JJC-2026110-001"));
        assert!(!is_legacy_id("JJC-20261101-001-2"));
        assert!(!is_legacy_id("ABC-20261101-001"));
    }

    #[test]
    fn tag_wird_aus_kennung_gelesen() {
        assert_eq!(
            legacy_id_day("JJC-20261101-001"),
            Some(NaiveDate::from_ymd_opt(2026, 11, 1).unwrap())
        );
        assert_eq!(legacy_id_day("JJC-ungueltig-001"), None);
    }
}
