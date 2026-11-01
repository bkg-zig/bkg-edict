//! Deutsche Anzeigetexte für Zustände, Prioritäten, Agenten und Amtsbezeichnungen.
//!
//! Die Datenbank hält die kanonischen Werte des Altbestands. Diese Tabelle
//! löst sie für die Anzeige auf. Neue Aufgaben werden bereits mit deutschen
//! Werten angelegt, damit die Datenbank keine zwei Sprachen mischt.

use crate::model::{FlowEntry, Task, TaskState};

/// Deutscher Name eines Zustands.
pub fn state_label(state: TaskState) -> &'static str {
    match state {
        TaskState::Taizi => "Taizi",
        TaskState::Zhongshu => "Zhongshu",
        TaskState::Menxia => "Menxia",
        TaskState::Assigned => "Zugewiesen",
        TaskState::Next => "Bereit",
        TaskState::Doing => "In Arbeit",
        TaskState::Review => "Prüfung",
        TaskState::Done => "Abgeschlossen",
        TaskState::Blocked => "Blockiert",
        TaskState::Cancelled => "Abgebrochen",
        TaskState::Pending => "Offen",
    }
}

/// Deutscher Name einer Priorität.
pub fn priority_label(priority: &str) -> &'static str {
    match priority {
        "low" | "niedrig" => "Niedrig",
        "high" | "hoch" => "Hoch",
        "urgent" | "dringend" => "Dringend",
        "中" => "Mittel",
        _ => "Normal",
    }
}

/// Reihenfolge der Zustände für die Anzeige im Board.
pub const BOARD_ORDER: [TaskState; 8] = [
    TaskState::Taizi,
    TaskState::Zhongshu,
    TaskState::Menxia,
    TaskState::Assigned,
    TaskState::Doing,
    TaskState::Review,
    TaskState::Done,
    TaskState::Blocked,
];

/// Definition einer Abteilung des Boards.
pub struct Department {
    pub key: &'static str,
    pub label: &'static str,
    pub icon: &'static str,
    pub action: &'static str,
}

/// Die Spalten des Kanban-Boards.
pub const DEPARTMENTS: &[Department] = &[
    Department {
        key: "Inbox",
        label: "Eingang",
        icon: "👑",
        action: "Anweisen",
    },
    Department {
        key: "Taizi",
        label: "Taizi",
        icon: "🤴",
        action: "Sortieren",
    },
    Department {
        key: "Zhongshu",
        label: "Zhongshu",
        icon: "📜",
        action: "Entwurf",
    },
    Department {
        key: "Menxia",
        label: "Menxia",
        icon: "🔍",
        action: "Prüfen",
    },
    Department {
        key: "Assigned",
        label: "Shangshu",
        icon: "📮",
        action: "Zuweisen",
    },
    Department {
        key: "Doing",
        label: "Sechs Ministerien",
        icon: "⚙️",
        action: "Ausführen",
    },
    Department {
        key: "Review",
        label: "Shangshu",
        icon: "🔎",
        action: "Sammeln",
    },
    Department {
        key: "Done",
        label: "Bericht",
        icon: "✅",
        action: "Abschließen",
    },
];

/// Kürzelt eine Zeichenkette auf die angegebene Länge.
pub fn truncate(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max_chars.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// Kanonische Organisationsbezeichnungen und ihre deutschen Namen.
pub const ORG_LABELS: &[(&str, &str)] = &[
    ("中书省", "Zhongshu"),
    ("门下省", "Menxia"),
    ("尚书省", "Shangshu"),
    ("六部", "Sechs Ministerien"),
    ("礼部", "Rites-Ministerium"),
    ("兵部", "Kriegsministerium"),
    ("户部", "Finanzministerium"),
    ("刑部", "Justizministerium"),
    ("工部", "Bauministerium"),
    ("吏部", "Personalministerium"),
    ("钦天监", "Astronomisches Amt"),
    ("皇上", "Kaiser"),
    ("太子", "Taizi"),
];

/// Kanonische Amtsbezeichnungen und ihre deutschen Namen.
pub const OFFICIAL_LABELS: &[(&str, &str)] = &[
    ("中书令", "Leiter der Zhongshu-Kanzlei"),
    ("侍中", "Palastprüfer"),
    ("尚书令", "Leiter der Shangshu-Kanzlei"),
    ("吏部尚书", "Personalminister"),
    ("户部尚书", "Finanzminister"),
    ("礼部尚书", "Rites-Minister"),
    ("兵部尚书", "Kriegsminister"),
    ("刑部尚书", "Justizminister"),
    ("工部尚书", "Bauminister"),
];

/// Weitere kanonische Werte, die in freien Textfeldern vorkommen.
pub const VALUE_LABELS: &[(&str, &str)] = &[
    ("无", "Keine"),
    ("执行中", "In Ausführung"),
    ("皇上", "Kaiser"),
    ("太子", "Taizi"),
];

/// Deutscher Name einer Organisation.
pub fn org_label(value: &str) -> String {
    lookup(ORG_LABELS, value)
}

/// Deutscher Name eines Amtsinhabers.
pub fn official_label(value: &str) -> String {
    lookup(OFFICIAL_LABELS, value)
}

/// Deutscher Name eines sonstigen kanonischen Wertes.
pub fn translate_value(value: &str) -> String {
    lookup(VALUE_LABELS, value)
}

/// Löst einen Wert in der Tabelle auf. Unbekannte Werte bleiben unverändert.
fn lookup(table: &[(&str, &str)], value: &str) -> String {
    table
        .iter()
        .find(|(source, _)| *source == value)
        .map(|(_, target)| (*target).to_string())
        .unwrap_or_else(|| value.to_string())
}

/// Übersetzt die sichtbaren Felder einer Aufgabe ins Deutsche.
///
/// Die Funktion arbeitet auf einer Kopie. Die Datenbank behält die
/// kanonischen Werte des Altbestands.
pub fn translate_task(task: &mut Task) {
    task.org = org_label(&task.org);
    task.official = official_label(&task.official);
    task.block = translate_value(&task.block);
    task.now = translate_value(&task.now);
    task.ac = translate_value(&task.ac);
    for entry in &mut task.flow_log {
        translate_flow_entry(entry);
    }
}

/// Übersetzt einen Eintrag des Zustandsprotokolls.
pub fn translate_flow_entry(entry: &mut FlowEntry) {
    if let Some(value) = entry.from.as_mut() {
        *value = translate_value(&org_label(value));
    }
    if let Some(value) = entry.to.as_mut() {
        *value = translate_value(&org_label(value));
    }
    if let Some(value) = entry.agent.as_mut() {
        *value = official_label(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn organisationen_werden_uebersetzt() {
        assert_eq!(org_label("中书省"), "Zhongshu");
        assert_eq!(org_label("工部"), "Bauministerium");
        assert_eq!(org_label("Unbekannt"), "Unbekannt");
    }

    #[test]
    fn amtsinhaber_werden_uebersetzt() {
        assert_eq!(official_label("中书令"), "Leiter der Zhongshu-Kanzlei");
        assert_eq!(official_label("尚书令"), "Leiter der Shangshu-Kanzlei");
    }

    #[test]
    fn werte_ohne_treffer_bleiben_unchanged() {
        assert_eq!(translate_value("无"), "Keine");
        assert_eq!(
            translate_value("Text mit mehr Worten"),
            "Text mit mehr Worten"
        );
    }

    #[test]
    fn aufgaben_werden_fuer_die_anzeige_uebersetzt() {
        let mut task = Task::new("JJC-1", "Test");
        task.org = "尚书省".to_string();
        task.official = "尚书令".to_string();
        task.block = "无".to_string();
        task.flow_log.push(FlowEntry {
            from: Some("皇上".to_string()),
            to: Some("中书省".to_string()),
            agent: Some("中书令".to_string()),
            ..FlowEntry::default()
        });
        translate_task(&mut task);
        assert_eq!(task.org, "Shangshu");
        assert_eq!(task.official, "Leiter der Shangshu-Kanzlei");
        assert_eq!(task.block, "Keine");
        let eintrag = task.flow_log.last().unwrap();
        assert_eq!(eintrag.from.as_deref(), Some("Kaiser"));
        assert_eq!(eintrag.to.as_deref(), Some("Zhongshu"));
        assert_eq!(
            eintrag.agent.as_deref(),
            Some("Leiter der Zhongshu-Kanzlei")
        );
    }
}
