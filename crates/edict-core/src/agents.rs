//! Agentenmetadaten der三省六部-Verwaltung.

/// Metadaten eines Agenten.
pub struct AgentMeta {
    pub id: &'static str,
    pub name: &'static str,
    pub role: &'static str,
    pub icon: &'static str,
}

/// Alle im System bekannten Agenten.
pub const AGENTS: &[AgentMeta] = &[
    AgentMeta {
        id: "zaochao",
        name: "Frühmorgensitzung",
        role: "Einberufung der Sitzung und Steuerung der Tagesordnung",
        icon: "🏛️",
    },
    AgentMeta {
        id: "shangshu",
        name: "Shangshu",
        role: "Gesamtsteuerung und Aufsicht über die Aufgaben",
        icon: "📜",
    },
    AgentMeta {
        id: "zhongshu",
        name: "Zhongshu",
        role: "Ausarbeiten von Erlassen und Planung der Vorgehensweise",
        icon: "✍️",
    },
    AgentMeta {
        id: "menxia",
        name: "Menxia",
        role: "Prüfung und Rücksendung beanstandeter Entwürfe",
        icon: "🔍",
    },
    AgentMeta {
        id: "libu",
        name: "Libu",
        role: "Personalverwaltung und Organisation",
        icon: "👤",
    },
    AgentMeta {
        id: "hubu",
        name: "Hubu",
        role: "Finanzen und Ressourcen",
        icon: "💰",
    },
    AgentMeta {
        id: "gongbu",
        name: "Gongbu",
        role: "Technische Umsetzung und Bau",
        icon: "🔧",
    },
    AgentMeta {
        id: "xingbu",
        name: "Xingbu",
        role: "Normen und Qualitätsprüfung",
        icon: "⚖️",
    },
    AgentMeta {
        id: "bingbu",
        name: "Bingbu",
        role: "Sicherheit und Notfallreaktion",
        icon: "🛡️",
    },
    AgentMeta {
        id: "libu_hr",
        name: "Libu Personal",
        role: "Personalführung, Teamausbau und Qualifizierung der Agenten",
        icon: "🧑‍🏫",
    },
    AgentMeta {
        id: "taizi",
        name: "Taizi",
        role: "Empfang aller Nachrichten, Sortierung und Weitergabe an die Zhongshu",
        icon: "👑",
    },
];

/// Metadaten eines Agenten nach Kennung.
pub fn find(id: &str) -> Option<&'static AgentMeta> {
    AGENTS.iter().find(|agent| agent.id == id)
}

/// Deutscher Anzeigename eines Agenten.
pub fn label(id: &str) -> String {
    find(id)
        .map(|agent| agent.name.to_string())
        .unwrap_or_else(|| id.to_string())
}
