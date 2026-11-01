//! Befehle für Betrieb, Berichte und Auswertung.

use anyhow::{anyhow, Result};
use serde_json::{json, Value};

use crate::api::ApiClient;
use crate::cli::{
    BriefArgs, DoctorArgs, InitArgs, SkillsArgs, SkillsCommand, SkillsListArgs, StatsArgs, SyncArgs,
};
use crate::commands::data::{extract_tasks, format_time, parse_date, text_field};
use crate::config;
use crate::output::{self, Cell};
use crate::state::STATES;

/// Legt die Konfiguration an und prüft den Server.
pub async fn init(url_override: Option<&str>, args: &InitArgs) -> Result<()> {
    let path = config::config_path();
    if path.exists() && !args.force {
        return Err(anyhow!(
            "Die Konfiguration {} existiert bereits. Mit --force wird sie überschrieben.",
            path.display()
        ));
    }

    let (url, source) = config::resolve_base_url(url_override.or(args.url.as_deref()))?;
    crate::api::parse_base_url(&url)?;
    let config_value = config::CliConfig {
        server_url: Some(url.clone()),
    };
    config::save_config(&config_value)?;

    println!("Konfiguration geschrieben: {}", path.display());
    println!("Server-Adresse: {url} (Quelle: {})", source.description());
    println!("Die Umgebungsvariable EDICT_API_URL oder EDICT_SERVER_URL überschreibt diese Datei.");

    let token = std::env::var("EDICT_ADMIN_TOKEN")
        .ok()
        .filter(|value| !value.trim().is_empty());
    let client = ApiClient::new(&url, token)?;
    match client.get("/healthz").await {
        Ok(value) => {
            let status = value
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("unbekannt");
            println!("Server antwortet auf /healthz: {status}");
            Ok(())
        }
        Err(error) => Err(anyhow!(
            "Der Server unter {url} ist nicht erreichbar: {error:#}. Die Konfiguration wurde geschrieben."
        )),
    }
}

/// Lädt den Live-Status und speichert ihn lokal.
pub async fn sync(client: &ApiClient, args: &SyncArgs) -> Result<()> {
    let status = client.get("/api/live-status").await?;
    let target = args
        .output
        .clone()
        .unwrap_or_else(config::default_status_path);
    config::write_json_with_lock(&target, &status)?;
    let count = status
        .get("tasks")
        .and_then(Value::as_array)
        .map(|tasks| tasks.len())
        .unwrap_or(0);
    println!(
        "Live-Status geschrieben: {} ({count} Aufgaben)",
        target.display()
    );
    Ok(())
}

/// Prüft Server, Datenbank und Gateway.
pub async fn doctor(client: &ApiClient, args: &DoctorArgs) -> Result<()> {
    let mut failures = 0;

    let health = client.get("/healthz").await;
    let database = client.get("/api/live-status").await;
    let gateway = client.get("/api/agents-status").await;

    let server_ok = health.is_ok();
    let database_ok = database.is_ok();
    let gateway_ok = gateway
        .as_ref()
        .ok()
        .and_then(|value| value.get("gateway"))
        .and_then(|entry| entry.get("alive"))
        .and_then(Value::as_bool)
        .unwrap_or(false);

    if output::wants_json(args.json) {
        let report = json!({
            "server": check_report(&health),
            "database": check_report(&database),
            "gateway": check_report(&gateway),
            "gatewayAlive": gateway_ok,
            "serverUrl": client.base_url().as_str(),
        });
        output::print_json(&report)?;
    } else {
        println!("Server: {}", check_text(&health));
        println!("Datenbank: {}", check_text(&database));
        println!("Gateway: {}", gateway_text(&gateway));
        println!("Server-Adresse: {}", client.base_url().as_str());
    }

    if !server_ok {
        failures += 1;
    }
    if !database_ok {
        failures += 1;
    }
    if failures > 0 {
        return Err(anyhow!(
            "Die Prüfung hat {failures} Fehler gefunden. Server und Datenbank müssen erreichbar sein."
        ));
    }
    Ok(())
}

/// Wandelt das Ergebnis einer Prüfung in einen JSON-Bericht.
fn check_report(result: &Result<Value>) -> Value {
    match result {
        Ok(value) => json!({ "ok": true, "detail": detail_of(value) }),
        Err(error) => json!({ "ok": false, "detail": format!("{error:#}") }),
    }
}

/// Deutscher Text zum Ergebnis einer Prüfung.
fn check_text(result: &Result<Value>) -> String {
    match result {
        Ok(value) => format!("erreichbar ({})", detail_of(value)),
        Err(error) => format!("nicht erreichbar: {error:#}"),
    }
}

/// Deutscher Text zum Gateway.
fn gateway_text(result: &Result<Value>) -> String {
    match result {
        Ok(value) => {
            let alive = value
                .get("gateway")
                .and_then(|entry| entry.get("alive"))
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let status = value
                .get("gateway")
                .map(|entry| text_field(entry, "status"))
                .unwrap_or_else(|| "-".to_string());
            if alive {
                format!("erreichbar ({status})")
            } else {
                format!("nicht erreichbar ({status})")
            }
        }
        Err(error) => format!("nicht erreichbar: {error:#}"),
    }
}

/// Kurzbeschreibung einer Antwort für den Bericht.
fn detail_of(value: &Value) -> String {
    if let Some(status) = value.get("status").and_then(Value::as_str) {
        return status.to_string();
    }
    if let Some(tasks) = value.get("tasks").and_then(Value::as_array) {
        return format!("{} Aufgaben", tasks.len());
    }
    if let Some(agents) = value.get("agents").and_then(Value::as_array) {
        return format!("{} Agenten", agents.len());
    }
    "-".to_string()
}

/// Zeigt den Morgenbrief.
pub async fn brief(client: &ApiClient, args: &BriefArgs) -> Result<()> {
    let route = match args.date.as_deref() {
        Some(date) => {
            let parsed = parse_date(date)?;
            format!("/api/morning-brief/{}", parsed.format("%Y%m%d"))
        }
        None => "/api/morning-brief".to_string(),
    };
    let value = client.get(&route).await?;
    if output::wants_json(args.json) {
        return output::print_json(&value);
    }

    if let Some(date) = value.get("date").and_then(Value::as_str) {
        println!("Morgenbrief vom {date}");
    }
    if let Some(generated) = value.get("generated_at").and_then(Value::as_str) {
        println!("Erstellt: {}", format_time(generated));
    }

    let categories = value
        .get("categories")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    if categories.is_empty() {
        println!();
        println!("Keine Kategorien gemeldet.");
        return Ok(());
    }

    for (name, items) in categories {
        println!();
        println!("{name}");
        let items = items.as_array().cloned().unwrap_or_default();
        if items.is_empty() {
            println!("  Keine Meldung.");
            continue;
        }
        for item in items {
            let title = text_field(&item, "title");
            let source = text_field(&item, "source");
            let link = item
                .get("link")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            if link.is_empty() {
                println!("  {title} ({source})");
            } else {
                println!("  {title} ({source}) {link}");
            }
        }
    }
    Ok(())
}

/// Zeigt Kennzahlen zu Agenten und Aufgaben.
pub async fn stats(client: &ApiClient, args: &StatsArgs) -> Result<()> {
    let officials = client.get("/api/officials-stats").await?;
    let live = client.get("/api/live-status").await.ok();

    if output::wants_json(args.json) {
        let report = json!({
            "officials": officials,
            "liveStatus": live,
        });
        return output::print_json(&report);
    }

    let entries = officials
        .get("officials")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if entries.is_empty() {
        println!("Keine Kennzahlen zu Amtsträgern gemeldet.");
    } else {
        let headers = vec![
            Cell::styled("Amt", output::BOLD),
            Cell::styled("Aufgaben erledigt", output::BOLD),
            Cell::styled("Aktive Aufgaben", output::BOLD),
            Cell::styled("Tokens ein", output::BOLD),
            Cell::styled("Tokens aus", output::BOLD),
            Cell::styled("Kosten CNY", output::BOLD),
            Cell::styled("Kosten USD", output::BOLD),
            Cell::styled("Sitzungen", output::BOLD),
        ];
        let rows: Vec<Vec<Cell>> = entries
            .iter()
            .map(|entry| {
                vec![
                    Cell::plain(edict_core::labels::official_label(&text_field(
                        entry, "label",
                    ))),
                    Cell::plain(count_text(entry, "tasks_done")),
                    Cell::plain(count_text(entry, "tasks_active")),
                    Cell::plain(count_text(entry, "tokens_in")),
                    Cell::plain(count_text(entry, "tokens_out")),
                    Cell::plain(cost_text(entry, "cost_cny")),
                    Cell::plain(cost_text(entry, "cost_usd")),
                    Cell::plain(count_text(entry, "sessions")),
                ]
            })
            .collect();
        print!("{}", output::render_table(&headers, &rows));
    }

    if let Some(totals) = officials.get("totals") {
        println!();
        println!(
            "Summe: {} erledigte Aufgaben, {} CNY",
            count_text(totals, "tasks_done"),
            cost_text(totals, "cost_cny")
        );
    }

    if let Some(live) = live.as_ref() {
        let tasks = extract_tasks(live)?;
        let mut counts: Vec<(String, usize)> = Vec::new();
        for state in STATES {
            let count = tasks
                .iter()
                .filter(|entry| entry.task.state == state)
                .count();
            counts.push((edict_core::labels::state_label(state).to_string(), count));
        }
        println!();
        println!("Aufgaben je Zustand");
        let headers = vec![
            Cell::styled("Zustand", output::BOLD),
            Cell::styled("Anzahl", output::BOLD),
        ];
        let rows: Vec<Vec<Cell>> = counts
            .iter()
            .map(|(state, count)| vec![Cell::plain(state.clone()), Cell::plain(count.to_string())])
            .collect();
        print!("{}", output::render_table(&headers, &rows));
    }
    Ok(())
}

/// Formatiert eine Zahl aus einem Objekt.
fn count_text(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_f64)
        .map(|number| format!("{number:.0}"))
        .unwrap_or_else(|| "-".to_string())
}

/// Formatiert eine Kostenangabe aus einem Objekt.
fn cost_text(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_f64)
        .map(|number| format!("{number:.2}"))
        .unwrap_or_else(|| "-".to_string())
}

/// Zeigt die Remote-Skills des Servers.
pub async fn skills(client: &ApiClient, args: &SkillsArgs) -> Result<()> {
    match &args.command {
        SkillsCommand::List(inner) => skills_list(client, inner).await,
    }
}

/// Listet die Remote-Skills auf.
pub async fn skills_list(client: &ApiClient, args: &SkillsListArgs) -> Result<()> {
    let value = client.get("/api/remote-skills-list").await?;
    if output::wants_json(args.json) {
        return output::print_json(&value);
    }

    let items = value
        .get("remoteSkills")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if items.is_empty() {
        println!("Kein Remote-Skill gemeldet.");
        return Ok(());
    }

    let headers = vec![
        Cell::styled("Skill", output::BOLD),
        Cell::styled("Agent", output::BOLD),
        Cell::styled("Quelle", output::BOLD),
        Cell::styled("Status", output::BOLD),
        Cell::styled("Hinzugefügt", output::BOLD),
    ];
    let rows: Vec<Vec<Cell>> = items
        .iter()
        .map(|item| {
            vec![
                Cell::plain(text_field(item, "skillName")),
                Cell::plain(text_field(item, "agentId")),
                Cell::plain(output::shorten(&text_field(item, "sourceUrl"), 48)),
                Cell::plain(text_field(item, "status")),
                Cell::plain(
                    item.get("addedAt")
                        .and_then(Value::as_str)
                        .map(format_time)
                        .unwrap_or_else(|| "-".to_string()),
                ),
            ]
        })
        .collect();
    print!("{}", output::render_table(&headers, &rows));
    println!("{} Remote-Skill(s)", items.len());
    Ok(())
}

/// Zeigt den Wire-Vertrag der Serverantworten.
pub fn protocol() -> Result<()> {
    print!("{}", protocol_text());
    Ok(())
}

/// Text des Wire-Vertrags der Serverantworten.
pub fn protocol_text() -> &'static str {
    "\
Wire-Vertrag der Serverantworten

Lesebefehle (GET)
  /healthz                 Antwort: { \"status\": \"ok\", \"ts\": \"...\", \"checks\": { ... } }
  /api/live-status         Antwort: { \"tasks\": [ ... ], \"syncStatus\": { \"ok\": true } }
  /api/task-activity/{id}  Antwort: { \"ok\": true, \"activity\": [ ... ] }
  /api/scheduler-state/{id} Antwort: { \"ok\": true, \"scheduler\": { ... } }
  /api/agents-status       Antwort: { \"ok\": true, \"gateway\": { ... }, \"agents\": [ ... ] }
  /api/agent-activity/{id} Antwort: { \"ok\": true, \"agentId\": \"...\", \"activity\": [ ... ] }
  /api/officials-stats     Antwort: { \"officials\": [ ... ], \"totals\": { ... } }
  /api/morning-brief       Antwort: { \"date\": \"...\", \"categories\": { ... } }
  /api/remote-skills-list  Antwort: { \"ok\": true, \"remoteSkills\": [ ... ] }

Schreibbefehle (POST)
  /api/create-task         Felder: task title, description, priority, assignee
  /api/task-action         Felder: task taskId, action, reason
  /api/advance-state       Felder: task taskId, to, agent, reason
  /api/archive-task        Felder: task taskId, archived
  /api/task-todos          Felder: task taskId, todos
  /api/task-progress       Felder: task taskId, agent, content
  /api/review-action       Felder: task taskId, action, comment

Antwort der Schreibbefehle
  Erfolg: { \"ok\": true, \"message\": \"...\" }
  Fehler: { \"ok\": false, \"error\": \"...\" } mit HTTP-Status ungleich 200

Die CLI wertet den HTTP-Status und das Feld \"ok\" aus. Im Fehlerfall enthält
die deutsche Meldung den Text des Servers.

Aufgabe (gekürzt)
  id, task_id, trace_id, title, description, state, org, official, now, eta,
  block, output, ac, priority, archived, flow_log, progress_log, todos, tags,
  meta, createdAt, updatedAt, archivedAt, sourceMeta, heartbeat, version

Zustände
  Taizi, Zhongshu, Menxia, Assigned, Next, Doing, Review, Done, Blocked,
  Cancelled, Pending

Teilaufgaben
  { \"id\": \"1\", \"title\": \"...\", \"status\": \"not-started|in-progress|completed\" }
"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zahlen_werden_formatiert() {
        let value = json!({"tasks_done": 4, "cost_cny": 1.5});
        assert_eq!(count_text(&value, "tasks_done"), "4");
        assert_eq!(cost_text(&value, "cost_cny"), "1.50");
        assert_eq!(count_text(&value, "fehlt"), "-");
    }

    #[test]
    fn pruefbericht_merkt_fehler() {
        let ok: Result<Value> = Ok(json!({"status": "ok"}));
        let fehler: Result<Value> = Err(anyhow!("keine Verbindung"));
        assert!(check_report(&ok)
            .get("ok")
            .and_then(Value::as_bool)
            .unwrap());
        assert!(!check_report(&fehler)
            .get("ok")
            .and_then(Value::as_bool)
            .unwrap());
        assert!(check_text(&fehler).contains("nicht erreichbar"));
    }

    #[test]
    fn protokoll_text_enthaelt_die_wichtigen_routen() {
        let text = protocol_text();
        assert!(text.contains("/api/live-status"));
        assert!(text.contains("/api/task-todos"));
        assert!(text.contains("/api/review-action"));
        assert!(text.contains("\"ok\": false"));
    }
}
