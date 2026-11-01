//! Deutsche Anzeigetexte für Zustände, Prioritäten und Agenten.

use crate::model::TaskState;

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
