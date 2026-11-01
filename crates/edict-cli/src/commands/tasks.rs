//! Befehle für Aufgaben.

use anyhow::{anyhow, Context, Result};
use edict_core::labels;
use edict_core::model::{Task, TaskState};
use serde_json::{json, Value};

use crate::api::{message_of, ApiClient};
use crate::cli::{
    ActionArgs, AdvanceArgs, ArchiveArgs, CreateArgs, ListArgs, ProgressArgs, ShowArgs, TaskArgs,
    TaskCommand, TasksArgs, TasksCommand, TodosArgs,
};
use crate::commands::data::{extract_tasks, find_task, format_time, parse_object, text_field};
use crate::output::{self, Cell};
use crate::state::parse_state;

/// Führt einen Aufgabenbefehl aus.
pub async fn run_tasks(client: &ApiClient, args: &TasksArgs) -> Result<()> {
    match &args.command {
        TasksCommand::List(inner) => list(client, inner).await,
        TasksCommand::Show(inner) => show(client, inner).await,
        TasksCommand::Create(inner) => create(client, inner).await,
        TasksCommand::Archive(inner) => archive(client, inner, true).await,
        TasksCommand::Unarchive(inner) => archive(client, inner, false).await,
    }
}

/// Führt einen Befehl für eine einzelne Aufgabe aus.
pub async fn run_task(client: &ApiClient, args: &TaskArgs) -> Result<()> {
    match &args.command {
        TaskCommand::Advance(inner) => advance(client, inner).await,
        TaskCommand::Action(inner) => action(client, inner).await,
        TaskCommand::Todos(inner) => todos(client, inner).await,
        TaskCommand::Progress(inner) => progress(client, inner).await,
    }
}

/// Listet Aufgaben auf.
pub async fn list(client: &ApiClient, args: &ListArgs) -> Result<()> {
    let live = client.get("/api/live-status").await?;
    let state = args
        .state
        .as_deref()
        .map(parse_state)
        .transpose()
        .context("Der Zustand konnte nicht gelesen werden.")?;
    let entries = extract_tasks(&live)?;
    let selected: Vec<_> = entries
        .iter()
        .filter(|entry| entry.task.archived == args.archived)
        .filter(|entry| state.is_none_or(|wanted| entry.task.state == wanted))
        .filter(|entry| {
            args.org
                .as_deref()
                .is_none_or(|org| matches_org(&entry.task, org))
        })
        .collect();

    if output::wants_json(args.json) {
        let raw: Vec<Value> = selected.iter().map(|entry| entry.raw.clone()).collect();
        let result = json!({
            "tasks": raw,
            "count": raw.len(),
            "archived": args.archived,
            "syncStatus": live.get("syncStatus").cloned().unwrap_or(Value::Null),
        });
        return output::print_json(&result);
    }

    if selected.is_empty() {
        println!("Keine Aufgabe gefunden.");
        return Ok(());
    }

    let headers = vec![
        Cell::styled("ID", output::BOLD),
        Cell::styled("Titel", output::BOLD),
        Cell::styled("Zustand", output::BOLD),
        Cell::styled("Org", output::BOLD),
        Cell::styled("Jetzt", output::BOLD),
        Cell::styled("Blockiert", output::BOLD),
        Cell::styled("Aktualisiert", output::BOLD),
    ];
    let rows: Vec<Vec<Cell>> = selected
        .iter()
        .map(|entry| {
            let task = &entry.task;
            vec![
                Cell::plain(task.id.clone()),
                Cell::plain(output::shorten(&task.title, 40)),
                Cell::styled(
                    labels::state_label(task.state).to_string(),
                    state_style(task.state),
                ),
                Cell::plain(labels::org_label(&task.org)),
                Cell::plain(output::shorten(&labels::translate_value(&task.now), 28)),
                Cell::plain(output::shorten(&labels::translate_value(&task.block), 28)),
                Cell::plain(format_time(&task.updated_at)),
            ]
        })
        .collect();
    print!("{}", output::render_table(&headers, &rows));
    println!("{} Aufgabe(n)", selected.len());
    Ok(())
}

/// Prüft, ob die Aufgabe zur angegebenen Organisation gehört.
fn matches_org(task: &Task, wanted: &str) -> bool {
    let wanted = wanted.trim();
    task.org.eq_ignore_ascii_case(wanted)
        || labels::org_label(&task.org).eq_ignore_ascii_case(wanted)
        || labels::translate_value(&task.org).eq_ignore_ascii_case(wanted)
}

/// Farbe für einen Zustand.
fn state_style(state: TaskState) -> &'static str {
    match state {
        TaskState::Done => output::GREEN,
        TaskState::Blocked | TaskState::Cancelled => output::RED,
        TaskState::Assigned | TaskState::Next | TaskState::Doing | TaskState::Review => {
            output::YELLOW
        }
        _ => output::CYAN,
    }
}

/// Zeigt eine Aufgabe ausführlich.
pub async fn show(client: &ApiClient, args: &ShowArgs) -> Result<()> {
    let live = client.get("/api/live-status").await?;
    let entry = find_task(&live, &args.id)?
        .ok_or_else(|| anyhow!("Die Aufgabe {} wurde nicht gefunden.", args.id))?;
    let task = &entry.task;

    if output::wants_json(args.json) {
        let mut result = entry.raw.clone();
        if let Some(map) = result.as_object_mut() {
            map.insert(
                "activity".to_string(),
                activity_of(client, &args.id).await.unwrap_or(Value::Null),
            );
        }
        return output::print_json(&result);
    }

    println!("Aufgabe {}", task.id);
    if !task.task_id.is_empty() {
        println!("  Technische Kennung: {}", task.task_id);
    }
    println!("  Titel: {}", task.title);
    println!("  Zustand: {}", labels::state_label(task.state));
    println!("  Organisation: {}", labels::org_label(&task.org));
    println!("  Amt: {}", labels::official_label(&task.official));
    println!("  Priorität: {}", labels::priority_label(&task.priority));
    println!("  Jetzt: {}", labels::translate_value(&task.now));
    println!("  Blockiert: {}", labels::translate_value(&task.block));
    println!("  Erwartet: {}", text_field(&entry.raw, "eta"));
    println!("  Abnahme: {}", text_field(&entry.raw, "ac"));
    println!("  Erstellt: {}", format_time(&task.created_at));
    println!("  Aktualisiert: {}", format_time(&task.updated_at));
    println!(
        "  Archiviert: {}",
        if task.archived { "ja" } else { "nein" }
    );
    if !task.description.trim().is_empty() {
        println!("  Beschreibung: {}", task.description.trim());
    }
    if !task.output.trim().is_empty() {
        println!("  Ausgabe: {}", task.output.trim());
    }

    if !task.flow_log.is_empty() {
        println!();
        println!("Zustandsprotokoll");
        let headers = vec![
            Cell::styled("Zeit", output::BOLD),
            Cell::styled("Von", output::BOLD),
            Cell::styled("Nach", output::BOLD),
            Cell::styled("Vermerk", output::BOLD),
            Cell::styled("Agent", output::BOLD),
        ];
        let rows: Vec<Vec<Cell>> = task
            .flow_log
            .iter()
            .map(|entry| {
                vec![
                    Cell::plain(entry.at.clone().unwrap_or_default()),
                    Cell::plain(labels::translate_value(&labels::org_label(
                        entry.from.as_deref().unwrap_or("-"),
                    ))),
                    Cell::plain(labels::translate_value(&labels::org_label(
                        entry.to.as_deref().unwrap_or("-"),
                    ))),
                    Cell::plain(output::shorten(entry.remark.as_deref().unwrap_or("-"), 40)),
                    Cell::plain(labels::official_label(
                        entry.agent.as_deref().unwrap_or("-"),
                    )),
                ]
            })
            .collect();
        print!("{}", output::render_table(&headers, &rows));
    }

    if !task.todos.is_empty() {
        println!();
        println!("Teilaufgaben");
        let headers = vec![
            Cell::styled("ID", output::BOLD),
            Cell::styled("Titel", output::BOLD),
            Cell::styled("Status", output::BOLD),
            Cell::styled("Detail", output::BOLD),
        ];
        let rows: Vec<Vec<Cell>> = task
            .todos
            .iter()
            .map(|todo| {
                vec![
                    Cell::plain(todo.id.clone()),
                    Cell::plain(todo.title.clone()),
                    Cell::plain(todo_status(&todo.status)),
                    Cell::plain(todo.detail.clone().unwrap_or_default()),
                ]
            })
            .collect();
        print!("{}", output::render_table(&headers, &rows));
    }

    if let Some(meta) = task.source_meta.as_ref() {
        println!();
        println!("Herkunft");
        let text = serde_json::to_string_pretty(meta)
            .context("Die Herkunftsangaben konnten nicht dargestellt werden.")?;
        println!("{text}");
    }

    match activity_of(client, &args.id).await {
        Ok(activity) => {
            let items = activity
                .get("activity")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            if !items.is_empty() {
                println!();
                println!("Aktivität");
                for item in items {
                    let kind = text_field(&item, "kind");
                    let agent = text_field(&item, "agent");
                    let text = item
                        .get("text")
                        .and_then(Value::as_str)
                        .or_else(|| item.get("thinking").and_then(Value::as_str))
                        .unwrap_or("");
                    println!("  [{kind}] {agent}: {}", output::shorten(text, 100));
                }
            }
        }
        Err(error) => {
            println!();
            println!("Aktivität nicht abrufbar: {error:#}");
        }
    }
    Ok(())
}

/// Deutscher Text für einen Teilaufgabenstatus.
fn todo_status(status: &str) -> &'static str {
    match status {
        "not-started" => "Offen",
        "in-progress" => "In Arbeit",
        "completed" => "Erledigt",
        _ => "Unbekannt",
    }
}

/// Liest die Aktivität einer Aufgabe.
async fn activity_of(client: &ApiClient, id: &str) -> Result<Value> {
    client
        .get(&format!("/api/task-activity/{}", id.trim()))
        .await
}

/// Legt eine Aufgabe an.
pub async fn create(client: &ApiClient, args: &CreateArgs) -> Result<()> {
    let title = args.title.trim();
    if title.is_empty() {
        return Err(anyhow!("Der Titel der Aufgabe darf nicht leer sein."));
    }
    let mut body = json!({ "title": title });
    if let Some(description) = args.description.as_deref() {
        body["description"] = json!(description);
    }
    if let Some(priority) = args.priority.as_deref() {
        body["priority"] = json!(priority.trim());
    }
    if let Some(assignee) = args.assignee.as_deref() {
        body["assignee"] = json!(assignee.trim());
    }
    let result = client.post("/api/create-task", &body).await?;
    let mut text = message_of(&result, "Die Aufgabe wurde angelegt.");
    if let Some(task_id) = result.get("taskId").and_then(Value::as_str) {
        text = format!("{text} (Kennung {task_id})");
    }
    println!("{text}");
    Ok(())
}

/// Archiviert eine Aufgabe oder holt sie aus dem Archiv.
pub async fn archive(client: &ApiClient, args: &ArchiveArgs, archived: bool) -> Result<()> {
    let body = json!({ "taskId": args.id.trim(), "archived": archived });
    let result = client.post("/api/archive-task", &body).await?;
    let fallback = if archived {
        "Die Aufgabe wurde archiviert."
    } else {
        "Die Aufgabe wurde aus dem Archiv geholt."
    };
    println!("{}", message_of(&result, fallback));
    Ok(())
}

/// Wechselt den Zustand einer Aufgabe.
pub async fn advance(client: &ApiClient, args: &AdvanceArgs) -> Result<()> {
    let target = parse_state(&args.to)?;
    let reason = args
        .reason
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("Zustandswechsel über die Kommandozeile");
    let body = json!({
        "taskId": args.id.trim(),
        "to": target.as_str(),
        "agent": args.agent.trim(),
        "reason": reason,
        "comment": reason,
    });
    let result = client.post("/api/advance-state", &body).await?;
    println!("{}", message_of(&result, "Der Zustand wurde gewechselt."));
    Ok(())
}

/// Führt eine Aktion auf einer Aufgabe aus.
pub async fn action(client: &ApiClient, args: &ActionArgs) -> Result<()> {
    let action = args.action.trim();
    if action.is_empty() {
        return Err(anyhow!("Die Aktion darf nicht leer sein."));
    }
    let mut body = json!({ "taskId": args.id.trim(), "action": action });
    if let Some(payload) = args.payload.as_deref() {
        let parsed = parse_object(payload, "--payload")?;
        if let Some(map) = parsed.as_object() {
            for (key, value) in map {
                body[key.as_str()] = value.clone();
            }
        }
    }
    let result = client.post("/api/task-action", &body).await?;
    println!("{}", message_of(&result, "Die Aktion wurde ausgeführt."));
    Ok(())
}

/// Setzt die Teilaufgaben einer Aufgabe.
pub async fn todos(client: &ApiClient, args: &TodosArgs) -> Result<()> {
    let parsed: Value = serde_json::from_str(&args.from)
        .context("Der Inhalt von --from ist kein gültiges JSON.")?;
    let list = match &parsed {
        Value::Array(items) => items.clone(),
        Value::Object(map) => match map.get("todos") {
            Some(Value::Array(items)) => items.clone(),
            _ => {
                return Err(anyhow!(
                    "Der Inhalt von --from muss ein JSON-Array oder ein Objekt mit dem Feld „todos“ sein."
                ))
            }
        },
        _ => {
            return Err(anyhow!(
                "Der Inhalt von --from muss ein JSON-Array oder ein Objekt mit dem Feld „todos“ sein."
            ))
        }
    };

    let mut items = Vec::with_capacity(list.len());
    for (index, item) in list.iter().enumerate() {
        let object = item.as_object().ok_or_else(|| {
            anyhow!(
                "Der Teilaufgabe an Position {} fehlt die Form eines JSON-Objekts.",
                index + 1
            )
        })?;
        let title = object
            .get("title")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow!("Der Teilaufgabe an Position {} fehlt der Titel.", index + 1))?;
        let id = object
            .get("id")
            .map(|value| match value {
                Value::String(text) => text.clone(),
                Value::Number(number) => number.to_string(),
                _ => (index + 1).to_string(),
            })
            .unwrap_or_else(|| (index + 1).to_string());
        let status = object
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("not-started")
            .to_string();
        let mut entry = json!({ "id": id, "title": title, "status": status });
        if let Some(detail) = object.get("detail").and_then(Value::as_str) {
            entry["detail"] = json!(detail);
        }
        items.push(entry);
    }

    let body = json!({ "taskId": args.id.trim(), "todos": items });
    let result = client.post("/api/task-todos", &body).await?;
    println!(
        "{}",
        message_of(&result, "Die Teilaufgaben wurden gesetzt.")
    );
    Ok(())
}

/// Hängt eine Fortschrittsmeldung an eine Aufgabe.
///
/// Der Server kann die Meldung über eine eigene Route oder über die
/// Aktionsroute annehmen. Beide Wege werden der Reihe nach versucht.
pub async fn progress(client: &ApiClient, args: &ProgressArgs) -> Result<()> {
    let content = args.content.trim();
    if content.is_empty() {
        return Err(anyhow!(
            "Der Inhalt der Fortschrittsmeldung darf nicht leer sein."
        ));
    }
    let body = json!({
        "taskId": args.id.trim(),
        "agent": args.agent.trim(),
        "content": content,
    });
    match client.post("/api/task-progress", &body).await {
        Ok(result) => {
            println!(
                "{}",
                message_of(&result, "Die Fortschrittsmeldung wurde gespeichert.")
            );
            Ok(())
        }
        Err(first) => {
            if !is_missing_route(&first) {
                return Err(first);
            }
            let fallback = json!({
                "taskId": args.id.trim(),
                "action": "progress",
                "agent": args.agent.trim(),
                "content": content,
                "reason": content,
            });
            match client.post("/api/task-action", &fallback).await {
                Ok(result) => {
                    println!(
                        "{}",
                        message_of(&result, "Die Fortschrittsmeldung wurde gespeichert.")
                    );
                    Ok(())
                }
                Err(second) => Err(anyhow!(
                    "Die Fortschrittsmeldung konnte nicht gespeichert werden. Die Route /api/task-progress antwortete mit {first}. Die Route /api/task-action antwortete mit {second}."
                )),
            }
        }
    }
}

/// Prüft, ob der Server die Route nicht kennt.
fn is_missing_route(error: &anyhow::Error) -> bool {
    let text = format!("{error:#}");
    text.contains("HTTP 404") || text.contains("nicht gefunden") || text.contains("unknown route")
}

#[cfg(test)]
mod tests {
    use super::*;
    use edict_core::model::TaskState;

    #[test]
    fn organisationen_werden_geprueft() {
        let mut task = Task::new("JJC-1", "Test");
        task.org = "尚书省".to_string();
        assert!(matches_org(&task, "尚书省"));
        assert!(matches_org(&task, "Shangshu"));
        assert!(matches_org(&task, "shangshu"));
        assert!(!matches_org(&task, "Taizi"));
    }

    #[test]
    fn zustaende_werden_eingefaerbt() {
        assert_eq!(state_style(TaskState::Done), output::GREEN);
        assert_eq!(state_style(TaskState::Blocked), output::RED);
        assert_eq!(state_style(TaskState::Doing), output::YELLOW);
        assert_eq!(state_style(TaskState::Taizi), output::CYAN);
    }

    #[test]
    fn teilaufgabenstatus_wird_uebersetzt() {
        assert_eq!(todo_status("not-started"), "Offen");
        assert_eq!(todo_status("in-progress"), "In Arbeit");
        assert_eq!(todo_status("completed"), "Erledigt");
        assert_eq!(todo_status("sonstiges"), "Unbekannt");
    }
}
