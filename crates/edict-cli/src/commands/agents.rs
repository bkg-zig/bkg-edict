//! Befehle für Agenten.

use anyhow::{anyhow, Result};
use edict_core::labels;
use serde_json::Value;

use crate::api::ApiClient;
use crate::cli::{ActivityArgs, AgentsArgs, AgentsCommand, StatusArgs};
use crate::commands::data::{format_time, text_field};
use crate::output::{self, Cell};

/// Führt einen Agentenbefehl aus.
pub async fn run(client: &ApiClient, args: &AgentsArgs) -> Result<()> {
    match &args.command {
        AgentsCommand::Status(inner) => status(client, inner).await,
        AgentsCommand::Activity(inner) => activity(client, inner).await,
    }
}

/// Zeigt den Status aller Agenten.
pub async fn status(client: &ApiClient, args: &StatusArgs) -> Result<()> {
    let value = client.get("/api/agents-status").await?;
    if output::wants_json(args.json) {
        return output::print_json(&value);
    }

    let gateway = value.get("gateway");
    let alive = gateway
        .and_then(|entry| entry.get("alive"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let gateway_status = gateway
        .map(|entry| text_field(entry, "status"))
        .unwrap_or_else(|| "-".to_string());
    println!(
        "Gateway: {}",
        if alive {
            "erreichbar"
        } else {
            "nicht erreichbar"
        }
    );
    println!("Gateway-Status: {gateway_status}");
    println!(
        "Geprüft: {}",
        value
            .get("checkedAt")
            .and_then(Value::as_str)
            .map(format_time)
            .unwrap_or_else(|| "-".to_string())
    );

    let agents = value
        .get("agents")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if agents.is_empty() {
        println!();
        println!("Kein Agent gemeldet.");
        return Ok(());
    }

    println!();
    let headers = vec![
        Cell::styled("Agent", output::BOLD),
        Cell::styled("Bezeichnung", output::BOLD),
        Cell::styled("Rolle", output::BOLD),
        Cell::styled("Status", output::BOLD),
        Cell::styled("Letzte Aktivität", output::BOLD),
    ];
    let rows: Vec<Vec<Cell>> = agents
        .iter()
        .map(|agent| {
            vec![
                Cell::plain(text_field(agent, "id")),
                Cell::plain(labels::official_label(&text_field(agent, "label"))),
                Cell::plain(text_field(agent, "role")),
                Cell::plain(text_field(agent, "statusLabel")),
                Cell::plain(
                    agent
                        .get("lastActive")
                        .and_then(Value::as_str)
                        .map(format_time)
                        .unwrap_or_else(|| "-".to_string()),
                ),
            ]
        })
        .collect();
    print!("{}", output::render_table(&headers, &rows));
    Ok(())
}

/// Zeigt die Aktivität eines Agenten.
pub async fn activity(client: &ApiClient, args: &ActivityArgs) -> Result<()> {
    let agent_id = args.agent_id.trim();
    if agent_id.is_empty() {
        return Err(anyhow!("Die Kennung des Agenten darf nicht leer sein."));
    }
    let value = client
        .get(&format!("/api/agent-activity/{}", agent_id))
        .await?;
    if output::wants_json(args.json) {
        return output::print_json(&value);
    }

    println!("Agent: {agent_id}");
    if let Some(label) = value.get("agentLabel").and_then(Value::as_str) {
        println!("Bezeichnung: {}", labels::official_label(label));
    }
    if let Some(last) = value.get("lastActive").and_then(Value::as_str) {
        println!("Letzte Aktivität: {}", format_time(last));
    }

    let entries = value
        .get("activity")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if entries.is_empty() {
        println!();
        println!("Keine Aktivität gemeldet.");
        return Ok(());
    }

    println!();
    let headers = vec![
        Cell::styled("Zeit", output::BOLD),
        Cell::styled("Art", output::BOLD),
        Cell::styled("Agent", output::BOLD),
        Cell::styled("Text", output::BOLD),
    ];
    let rows: Vec<Vec<Cell>> = entries
        .iter()
        .map(|entry| {
            vec![
                Cell::plain(
                    entry
                        .get("at")
                        .and_then(Value::as_str)
                        .map(format_time)
                        .unwrap_or_else(|| "-".to_string()),
                ),
                Cell::plain(text_field(entry, "kind")),
                Cell::plain(text_field(entry, "agent")),
                Cell::plain(output::shorten(
                    entry
                        .get("text")
                        .and_then(Value::as_str)
                        .or_else(|| entry.get("thinking").and_then(Value::as_str))
                        .unwrap_or("-"),
                    60,
                )),
            ]
        })
        .collect();
    print!("{}", output::render_table(&headers, &rows));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bezeichnungen_werden_uebersetzt() {
        assert_eq!(
            labels::official_label("中书令"),
            "Leiter der Zhongshu-Kanzlei"
        );
    }
}
