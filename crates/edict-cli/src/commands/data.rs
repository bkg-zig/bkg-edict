//! Gemeinsame Hilfen für die Kommandos.

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Local, NaiveDate};
use edict_core::model::Task;
use serde_json::Value;

/// Aufgabe mit ihrem unveränderten JSON-Inhalt.
#[derive(Debug, Clone)]
pub struct TaskEntry {
    /// Rohwert aus der Serverantwort.
    pub raw: Value,
    /// Gelesene Aufgabe.
    pub task: Task,
}

/// Liest die Aufgabenliste aus einer Live-Status-Antwort.
pub fn extract_tasks(live: &Value) -> Result<Vec<TaskEntry>> {
    let list = live
        .get("tasks")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("Die Antwort des Servers enthält kein Feld „tasks“."))?;
    let mut entries = Vec::with_capacity(list.len());
    for (index, raw) in list.iter().enumerate() {
        let task: Task = serde_json::from_value(raw.clone()).map_err(|error| {
            anyhow!(
                "Die Aufgabe an Position {} konnte nicht gelesen werden: {error}",
                index + 1
            )
        })?;
        entries.push(TaskEntry {
            raw: raw.clone(),
            task,
        });
    }
    Ok(entries)
}

/// Sucht eine Aufgabe über fachliche Kennung, technische Kennung oder Spur.
pub fn find_task(live: &Value, id: &str) -> Result<Option<TaskEntry>> {
    let wanted = id.trim();
    Ok(extract_tasks(live)?.into_iter().find(|entry| {
        entry.task.id == wanted || entry.task.task_id == wanted || entry.task.trace_id == wanted
    }))
}

/// Formatiert einen Zeitstempel für die Anzeige.
///
/// Unbekannte Formate bleiben unverändert, damit kein Wert verloren geht.
pub fn format_time(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return "-".to_string();
    }
    match DateTime::parse_from_rfc3339(trimmed) {
        Ok(parsed) => parsed
            .with_timezone(&Local)
            .format("%d.%m.%Y %H:%M")
            .to_string(),
        Err(_) => trimmed.to_string(),
    }
}

/// Prüft ein Datum im Format JJJJ-MM-TT oder JJJJMMTT.
pub fn parse_date(value: &str) -> Result<NaiveDate> {
    let trimmed = value.trim();
    for format in ["%Y-%m-%d", "%Y%m%d"] {
        if let Ok(date) = NaiveDate::parse_from_str(trimmed, format) {
            return Ok(date);
        }
    }
    Err(anyhow!(
        "Das Datum „{trimmed}“ ist ungültig. Erlaubt sind JJJJ-MM-TT und JJJJMMTT."
    ))
}

/// Liest einen Text aus einem JSON-Objekt.
pub fn text_field(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .unwrap_or("-")
        .to_string()
}

/// Prüft, ob ein JSON-Text ein Obekt ist, und gibt es zurück.
pub fn parse_object(text: &str, label: &str) -> Result<Value> {
    let parsed: Value = serde_json::from_str(text)
        .with_context(|| format!("Der Inhalt von {label} ist kein gültiges JSON."))?;
    if !parsed.is_object() {
        return Err(anyhow!("Der Inhalt von {label} muss ein JSON-Objekt sein."));
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn aufgaben_werden_gelesen() {
        let live = json!({
            "tasks": [
                {"id": "JJC-1", "title": "Erste", "state": "Doing"},
                {"id": "JJC-2", "title": "Zweite", "state": "Done"}
            ],
            "syncStatus": {"ok": true}
        });
        let entries = extract_tasks(&live).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].task.id, "JJC-1");
    }

    #[test]
    fn fehlende_aufgabenliste_wird_gemeldet() {
        let error = extract_tasks(&json!({"syncStatus": {}})).unwrap_err();
        assert!(error.to_string().contains("tasks"));
    }

    #[test]
    fn aufgabe_wird_ueber_alle_kennungen_gefunden() {
        let live = json!({
            "tasks": [{
                "id": "JJC-1",
                "task_id": "uuid-1",
                "trace_id": "trace-1",
                "title": "Erste",
                "state": "Doing"
            }]
        });
        assert!(find_task(&live, "JJC-1").unwrap().is_some());
        assert!(find_task(&live, "uuid-1").unwrap().is_some());
        assert!(find_task(&live, "trace-1").unwrap().is_some());
        assert!(find_task(&live, "fehlt").unwrap().is_none());
    }

    #[test]
    fn datumsangaben_werden_geprueft() {
        assert!(parse_date("2026-10-31").is_ok());
        assert!(parse_date("20261031").is_ok());
        assert!(parse_date("31.10.2026").is_err());
    }

    #[test]
    fn zeitstempel_bleiben_unveraendert_bei_unbekanntem_format() {
        assert_eq!(format_time(""), "-");
        assert_eq!(format_time("gestern"), "gestern");
    }
}
