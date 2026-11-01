//! Uebersichten zu Agenten, Statistik und Berichten.

use dioxus::prelude::*;
use serde_json::{Map, Value};

use crate::app::{Action, Command, UiContext};
use crate::models::{
    agent_status_label, format_currency, format_number, format_timestamp, official_label,
    org_label, truncate, ActivityEntry, AgentActivityData, AgentConfig, AgentInfo, AgentStatusInfo,
    AgentsStatusData, ChangeLogEntry, KnownModel, MorningBrief, MorningNewsItem, OfficialInfo,
    OfficialsData,
};

/// Deutsche Bezeichnungen fuer die Kategorien des Morgenberichts.
const CATEGORY_LABELS: &[(&str, &str)] = &[
    ("政治", "Politik"),
    ("军事", "Militaer"),
    ("经济", "Wirtschaft"),
    ("AI大模型", "KI-Modelle"),
];

/// Deutscher Name einer Kategorie des Morgenberichts.
fn category_label(category: &str) -> String {
    CATEGORY_LABELS
        .iter()
        .find(|(source, _)| *source == category)
        .map(|(_, target)| (*target).to_string())
        .unwrap_or_else(|| category.to_string())
}

/// Deutsche Bezeichnungen fuer die Felder des letzten Ergebnisses.
fn result_key_label(key: &str) -> String {
    match key {
        "at" => "Zeitpunkt".to_string(),
        "applied" => "Angewendete Aenderungen".to_string(),
        "errors" => "Fehler".to_string(),
        "gatewayRestarted" => "Gateway neu gestartet".to_string(),
        "rolledBack" => "Zurueckgerollt".to_string(),
        _ => format!("Angabe ({key})"),
    }
}

/// Formt einen Aktivitaetseintrag eines Agenten in eine Zeile.
fn activity_line(entry: &ActivityEntry) -> String {
    let text = entry
        .text
        .clone()
        .or_else(|| entry.thinking.clone())
        .or_else(|| entry.output.clone())
        .or_else(|| entry.remark.clone())
        .unwrap_or_else(|| entry.kind.clone());
    if text.is_empty() {
        return "Kein Text".to_string();
    }
    truncate(&text, 240)
}

/// Formt den Zeitstempel eines Aktivitaetseintrags.
fn activity_time(entry: &ActivityEntry) -> String {
    let raw = match &entry.at {
        Some(serde_json::Value::String(text)) => text.clone(),
        Some(serde_json::Value::Number(number)) => number.to_string(),
        _ => String::new(),
    };
    if raw.is_empty() {
        return String::new();
    }
    let formatted = format_timestamp(&raw);
    if formatted.is_empty() {
        raw
    } else {
        formatted
    }
}

/// Zeile eines Agenten in der Statusliste.
fn agent_row(agent: &AgentStatusInfo) -> Element {
    let label = org_label(&agent.label);
    let role = official_label(&agent.role);
    rsx! {
        tr {
            td { {label} }
            td { {role} }
            td {
                span {
                    class: match agent.status.as_str() {
                        "running" => "badge badge-success",
                        "idle" => "badge badge-accent",
                        "offline" => "badge",
                        _ => "badge badge-warning",
                    },
                    {agent_status_label(&agent.status)}
                }
            }
            td { {agent.last_active.clone().unwrap_or_default()} }
        }
    }
}

/// Schaltflaeche, die die Aktivitaet eines Agenten laedt.
fn activity_button(context: &UiContext, agent: &AgentStatusInfo) -> Element {
    let agent_id = agent.id.clone();
    let commands = context.commands.clone();
    let label = format!("Aktivitaet von {}", org_label(&agent.label));
    rsx! {
        button {
            class: "secondary-button",
            r#type: "button",
            aria_label: "{label}",
            onclick: move |_| {
                let _ = commands.unbounded_send(Command::LoadAgentActivity(agent_id.clone()));
            },
            "{label}"
        }
    }
}

/// Zeile eines Amtsinhabers in der Statistik.
fn official_row(official: &OfficialInfo) -> Element {
    let office = if official.role.is_empty() {
        org_label(&official.label)
    } else {
        official_label(&official.role)
    };
    rsx! {
        tr {
            td { {office} }
            td { {format_number(official.tasks_done)} }
            td { {format_number(official.tasks_active)} }
            td { {format_number(official.messages)} }
            td { {format_number(official.tokens_in + official.tokens_out)} }
            td { {format_currency(official.cost_cny)} }
            td { {format_number(official.merit_score as i64)} }
        }
    }
}

/// Zeile einer Agentenkonfiguration.
fn config_row(agent: &AgentInfo) -> Element {
    let label = org_label(&agent.label);
    let role = official_label(&agent.role);
    rsx! {
        tr {
            td { {label} }
            td { {agent.id.clone()} }
            td { {role} }
            td { {agent.model.clone()} }
            td { "{agent.skills.len()}" }
        }
    }
}

/// Zeile eines Eintrags im Morgenbericht.
fn news_row(item: &MorningNewsItem) -> Element {
    let summary = item
        .summary
        .clone()
        .or_else(|| item.desc.clone())
        .map(|text| truncate(&text, 240));
    rsx! {
        div { class: "news-item",
            div { class: "news-title", {item.title.clone()} }
            if let Some(summary) = summary {
                div { {summary} }
            }
            div { class: "news-meta",
                "{item.source.clone()} {item.pub_date.clone().unwrap_or_default()}"
            }
        }
    }
}

/// Schaltflaeche, die einen Agenten aufweckt.
fn wake_button(context: &UiContext, agent: &AgentInfo) -> Element {
    let agent_id = agent.id.clone();
    let commands = context.commands.clone();
    let label = format!("{} aufwecken", org_label(&agent.label));
    rsx! {
        button {
            class: "secondary-button",
            r#type: "button",
            aria_label: "{label}",
            onclick: move |_| {
                let _ = commands.unbounded_send(Command::Action(Action::Wake {
                    agent_id: agent_id.clone(),
                    message: String::new(),
                }));
            },
            "{label}"
        }
    }
}

/// Auswahl des neuen Modells eines Agenten.
fn model_select(context: &UiContext, agent: &AgentInfo, models: &[KnownModel]) -> Element {
    let agent_id = agent.id.clone();
    let commands = context.commands.clone();
    let label = format!("Neues Modell fuer {}", org_label(&agent.label));
    rsx! {
        div { class: "action-row",
            span { class: "task-card-line", {org_label(&agent.label)} }
            select {
                aria_label: "{label}",
                onchange: move |event| {
                    let model = event.value();
                    if model.is_empty() {
                        return;
                    }
                    let _ = commands.unbounded_send(Command::Action(Action::SetModel {
                        agent_id: agent_id.clone(),
                        model,
                    }));
                },
                option { value: "", "Modell waehlen" }
                for model in models.iter() {
                    option { value: model.id.clone(), {model.label.clone()} }
                }
            }
        }
    }
}

/// Anzeige des Agentenstatus.
fn agents_panel(context: &UiContext, data: Option<AgentsStatusData>) -> Element {
    let Some(data) = data else {
        return rsx! { p { "Der Agentenstatus wurde noch nicht geladen." } };
    };

    rsx! {
        div { class: "stat-row",
            div { class: "stat",
                div { class: "stat-value", "{data.agents.len()}" }
                div { class: "stat-label", "Agenten" }
            }
            div { class: "stat",
                div { class: "stat-value",
                    if data.gateway.alive { "aktiv" } else { "inaktiv" }
                }
                div { class: "stat-label", "Gateway" }
            }
        }
        if data.agents.is_empty() {
            p { "Der Server meldet keine Agenten." }
        } else {
            div { class: "table-wrapper",
                table { class: "table",
                    thead {
                        tr {
                            th { "Agent" }
                            th { "Rolle" }
                            th { "Status" }
                            th { "Zuletzt aktiv" }
                        }
                    }
                    tbody {
                        for agent in data.agents.iter() {
                            {agent_row(agent)}
                        }
                    }
                }
            }
            div { class: "action-row",
                for agent in data.agents.iter() {
                    {activity_button(context, agent)}
                }
            }
        }
        p { class: "task-card-line", "Geprueft am {data.checked_at}" }
    }
}

/// Anzeige der Aktivitaet eines Agenten.
fn activity_panel(data: Option<AgentActivityData>, selected_agent: &str) -> Element {
    let Some(data) = data else {
        return rsx! { p { "Die Aktivitaet wurde noch nicht geladen." } };
    };

    rsx! {
        if selected_agent.is_empty() {
            p { "Waehlen Sie einen Agenten im Status, um seine Aktivitaet zu sehen." }
        } else {
            p { class: "task-card-line", "Agent: {selected_agent}" }
        }
        if data.activity.is_empty() {
            p { "Fuer diesen Agenten liegt keine Aktivitaet vor." }
        } else {
            ul { class: "log-list",
                for entry in data.activity.iter().rev().take(50) {
                    li { class: "log-item",
                        div { class: "log-time", {activity_time(entry)} }
                        div { {activity_line(entry)} }
                    }
                }
            }
        }
    }
}

/// Anzeige der amtlichen Statistik.
fn officials_panel(data: Option<OfficialsData>) -> Element {
    let Some(data) = data else {
        return rsx! { p { "Die Statistik wurde noch nicht geladen." } };
    };

    rsx! {
        div { class: "stat-row",
            div { class: "stat",
                div { class: "stat-value", "{format_number(data.totals.tasks_done)}" }
                div { class: "stat-label", "Abgeschlossene Aufgaben" }
            }
            div { class: "stat",
                div { class: "stat-value", "{format_currency(data.totals.cost_cny)}" }
                div { class: "stat-label", "Kosten" }
            }
        }
        if data.officials.is_empty() {
            p { "Die Server meldet keine Amtlichen." }
        } else {
            div { class: "table-wrapper",
                table { class: "table",
                    thead {
                        tr {
                            th { "Amt" }
                            th { "Erledigt" }
                            th { "Aktiv" }
                            th { "Nachrichten" }
                            th { "Token" }
                            th { "Kosten" }
                            th { "Leistung" }
                        }
                    }
                    tbody {
                        for official in data.officials.iter() {
                            {official_row(official)}
                        }
                    }
                }
            }
        }
    }
}

/// Anzeige des Morgenberichts.
fn morning_panel(data: Option<MorningBrief>) -> Element {
    let Some(data) = data else {
        return rsx! { p { "Der Morgenbericht wurde noch nicht geladen." } };
    };

    rsx! {
        if let Some(date) = data.date.clone() {
            p { class: "task-card-line", "Datum: {date}" }
        }
        if data.categories.is_empty() {
            p { "Der Morgenbericht enthaelt keine Eintraege." }
        } else {
            for (category, items) in data.categories.iter() {
                h4 { {category_label(category)} }
                for item in items.iter() {
                    {news_row(item)}
                }
            }
        }
    }
}

/// Anzeige der Agentenkonfiguration.
fn config_panel(context: &UiContext, data: Option<AgentConfig>) -> Element {
    let Some(data) = data else {
        return rsx! { p { "Die Agentenkonfiguration wurde noch nicht geladen." } };
    };

    rsx! {
        if data.agents.is_empty() {
            p { "Der Server meldet keine konfigurierten Agenten." }
        } else {
            div { class: "table-wrapper",
                table { class: "table",
                    thead {
                        tr {
                            th { "Agent" }
                            th { "Kennung" }
                            th { "Rolle" }
                            th { "Modell" }
                            th { "Skills" }
                        }
                    }
                    tbody {
                        for agent in data.agents.iter() {
                            {config_row(agent)}
                        }
                    }
                }
            }
            div { class: "action-row",
                for agent in data.agents.iter() {
                    {wake_button(context, agent)}
                }
            }
            h4 { "Modell wechseln" }
            for agent in data.agents.iter() {
                {model_select(context, agent, &data.known_models)}
            }
        }
    }
}

/// Anzeige des Protokolls der Modellwechsel.
fn model_log_panel(entries: &[ChangeLogEntry]) -> Element {
    if entries.is_empty() {
        return rsx! { p { "Es wurde kein Modellwechsel aufgezeichnet." } };
    }

    rsx! {
        div { class: "table-wrapper",
            table { class: "table",
                thead {
                    tr {
                        th { "Zeitpunkt" }
                        th { "Agent" }
                        th { "Vorher" }
                        th { "Nachher" }
                        th { "Zurueckgerollt" }
                    }
                }
                tbody {
                    for entry in entries.iter() {
                        tr {
                            td { {format_timestamp(&entry.at)} }
                            td { {entry.agent_id.clone()} }
                            td { {entry.old_model.clone()} }
                            td { {entry.new_model.clone()} }
                            td { if entry.rolled_back { "ja" } else { "nein" } }
                        }
                    }
                }
            }
        }
    }
}

/// Anzeige des letzten Ergebnisses.
fn result_panel(data: Option<Map<String, Value>>) -> Element {
    let Some(data) = data else {
        return rsx! { p { "Das Ergebnis wurde noch nicht geladen." } };
    };

    if data.is_empty() {
        return rsx! { p { "Der Server meldet kein Ergebnis." } };
    }

    rsx! {
        dl { class: "result-list",
            for (key, value) in data.iter() {
                dt { {result_key_label(key)} }
                dd { {result_value(value)} }
            }
        }
    }
}

/// Formt einen Wert des letzten Ergebnisses in lesbaren Text.
fn result_value(value: &Value) -> String {
    match value {
        Value::Null => "Keine Angabe".to_string(),
        Value::Bool(flag) => {
            if *flag {
                "Ja".to_string()
            } else {
                "Nein".to_string()
            }
        }
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Array(items) => {
            if items.is_empty() {
                return "Keine Eintraege".to_string();
            }
            items
                .iter()
                .map(|item| match item {
                    Value::String(text) => text.clone(),
                    other => serde_json::to_string(other).unwrap_or_default(),
                })
                .collect::<Vec<String>>()
                .join(", ")
        }
        Value::Object(fields) => {
            if fields.is_empty() {
                return "Keine Angabe".to_string();
            }
            fields
                .iter()
                .map(|(key, value)| format!("{}: {}", result_key_label(key), result_value(value)))
                .collect::<Vec<String>>()
                .join(", ")
        }
    }
}

/// Die Uebersichten der Oberflaeche.
#[component]
pub fn Overview() -> Element {
    let context = use_context::<UiContext>();
    let state = context.state;

    let agents_status = state.agents_status.read().clone();
    let officials = state.officials.read().clone();
    let morning_brief = state.morning_brief.read().clone();
    let agent_config = state.agent_config.read().clone();
    let model_log = state.model_log.read().clone();
    let last_result = state.last_result.read().clone();
    let agent_activity = state.agent_activity.read().clone();
    let selected_agent = state.selected_agent.read().clone();

    let reload = {
        let commands = context.commands.clone();
        move |_| {
            let _ = commands.unbounded_send(Command::LoadOverview);
        }
    };

    rsx! {
        section { class: "overview-view",
            div { class: "action-row",
                h2 { "Uebersichten" }
                button {
                    class: "secondary-button",
                    r#type: "button",
                    onclick: reload,
                    "Uebersichten neu laden"
                }
            }
            div { class: "overview-grid",
                div { class: "panel",
                    h3 { "Agentenstatus" }
                    {agents_panel(&context, agents_status)}
                }
                div { class: "panel",
                    h3 { "Agentenaktivitaet" }
                    {activity_panel(agent_activity, &selected_agent)}
                }
                div { class: "panel",
                    h3 { "Amtliche Statistik" }
                    {officials_panel(officials)}
                }
                div { class: "panel",
                    h3 { "Morgenbericht" }
                    {morning_panel(morning_brief)}
                }
                div { class: "panel",
                    h3 { "Agentenkonfiguration" }
                    {config_panel(&context, agent_config)}
                }
                div { class: "panel",
                    h3 { "Modellwechsel-Protokoll" }
                    {model_log_panel(&model_log)}
                }
                div { class: "panel",
                    h3 { "Letztes Ergebnis" }
                    {result_panel(last_result)}
                }
            }
        }
    }
}
