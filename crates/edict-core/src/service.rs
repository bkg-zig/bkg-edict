//! Fachliche Regeln, die auf Aufgaben ohne Datenbankzugriff angewendet werden.
//!
//! Server, Worker und CLI laden eine Aufgabe, ändern sie über diese Funktionen
//! und speichern sie wieder. Die Regeln stehen damit an einer Stelle und
//! werden von jeder Komponente gleich angewendet.

use chrono::Utc;
use serde_json::{Map, Value};

use crate::error::{AppError, AppResult};
use crate::model::{
    topics, Event, FlowEntry, ProgressEntry, Task, TaskState, TodoItem, TODO_STATUSES,
};
use crate::state_machine::{allowed_transitions, can_transition, is_terminal};

/// Wechselt den Zustand einer Aufgabe und protokolliert den Übergang.
///
/// Der Übergang wird gegen die Zustandsmaschine geprüft. Ist er nicht
/// erlaubt, bleibt die Aufgabe unverändert und es entsteht ein Fehler mit der
/// Liste der erlaubten Folgezustände.
pub fn apply_transition(
    task: &mut Task,
    new_state: TaskState,
    agent: &str,
    reason: &str,
) -> AppResult<()> {
    let from = task.state;
    if !can_transition(from, new_state) {
        return Err(AppError::InvalidTransition {
            from: from.as_str().to_string(),
            to: new_state.as_str().to_string(),
            allowed: allowed_transitions(from)
                .iter()
                .map(|state| state.as_str().to_string())
                .collect(),
        });
    }

    let now = Utc::now().to_rfc3339();
    task.prev_state = Some(from.as_str().to_string());
    task.state = new_state;
    task.flow_log.push(FlowEntry {
        at: Some(now.clone()),
        from: Some(from.as_str().to_string()),
        to: Some(new_state.as_str().to_string()),
        remark: Some(reason.to_string()),
        agent: Some(agent.to_string()),
        reason: Some(reason.to_string()),
        ts: Some(now.clone()),
    });
    task.updated_at = now;
    Ok(())
}

/// Hängt einen Fortschrittseintrag an und liefert ihn zurück.
pub fn apply_progress(
    task: &mut Task,
    agent: &str,
    content: &str,
    agent_label: Option<&str>,
) -> ProgressEntry {
    let now = Utc::now().to_rfc3339();
    let entry = ProgressEntry {
        at: Some(now.clone()),
        agent: Some(agent.to_string()),
        agent_label: agent_label.map(str::to_string),
        text: Some(content.to_string()),
        content: Some(content.to_string()),
        todos: Vec::new(),
        state: Some(task.state.as_str().to_string()),
        org: Some(task.org.clone()),
        tokens: None,
        cost: None,
        elapsed: None,
        ts: Some(now.clone()),
    };
    task.progress_log.push(entry.clone());
    task.updated_at = now;
    entry
}

/// Setzt die Teilaufgaben und normalisiert ihre Zustände.
///
/// Unbekannte Zustände werden auf `not-started` gesetzt, weil die Datenbank
/// nur die drei bekannten Werte zulässt.
pub fn apply_todos(task: &mut Task, todos: Vec<TodoItem>) {
    let normalized = todos
        .into_iter()
        .enumerate()
        .map(|(index, mut todo)| {
            if !TODO_STATUSES.contains(&todo.status.as_str()) {
                todo.status = "not-started".to_string();
            }
            if todo.id.trim().is_empty() {
                todo.id = (index + 1).to_string();
            }
            todo
        })
        .collect();
    task.todos = normalized;
    task.touch();
}

/// Setzt die Scheduler-Angaben einer Aufgabe.
pub fn apply_scheduler(task: &mut Task, scheduler: Map<String, Value>) {
    task.scheduler = scheduler;
    task.touch();
}

/// Baut das Ereignis, das bei einer neu angelegten Aufgabe entsteht.
///
/// Die Nutzlast entspricht dem bisherigen Vertrag: Kennung, Titel, Zustand,
/// Priorität und übernehmende Organisation.
pub fn task_created_event(task: &Task) -> Event {
    let mut payload = Map::new();
    payload.insert("task_id".into(), Value::String(task.task_id.clone()));
    payload.insert("title".into(), Value::String(task.title.clone()));
    payload.insert(
        "state".into(),
        Value::String(task.state.as_str().to_string()),
    );
    payload.insert("priority".into(), Value::String(task.priority.clone()));
    payload.insert(
        "assignee_org".into(),
        match &task.assignee_org {
            Some(org) => Value::String(org.clone()),
            None => Value::Null,
        },
    );
    build_event(
        task,
        topics::TASK_CREATED,
        "task.created",
        "task_service",
        payload,
    )
}

/// Baut das Ereignis eines Zustandswechsels.
///
/// Endzustände laufen über `task.completed`, alle anderen über `task.status`.
/// Der Ereignistyp benennt den neuen Zustand, die Nutzlast hält Quelle, Ziel
/// und Begründung.
pub fn state_change_event(
    task: &Task,
    from: TaskState,
    to: TaskState,
    reason: &str,
    agent: &str,
) -> Event {
    let topic = if is_terminal(to) {
        topics::TASK_COMPLETED
    } else {
        topics::TASK_STATUS
    };
    let mut payload = Map::new();
    payload.insert("task_id".into(), Value::String(task.task_id.clone()));
    payload.insert("from".into(), Value::String(from.as_str().to_string()));
    payload.insert("to".into(), Value::String(to.as_str().to_string()));
    payload.insert("reason".into(), Value::String(reason.to_string()));
    build_event(
        task,
        topic,
        &format!("task.state.{}", to.as_str()),
        agent,
        payload,
    )
}

/// Baut das Ereignis, das eine Ausführung beim Gateway anfordert.
pub fn dispatch_event(task: &Task, agent: &str, message: &str) -> Event {
    let mut payload = Map::new();
    payload.insert("task_id".into(), Value::String(task.task_id.clone()));
    payload.insert("agent".into(), Value::String(agent.to_string()));
    payload.insert("message".into(), Value::String(message.to_string()));
    payload.insert(
        "state".into(),
        Value::String(task.state.as_str().to_string()),
    );
    build_event(
        task,
        topics::TASK_DISPATCH,
        "task.dispatch.request",
        "task_service",
        payload,
    )
}

/// Baut das Ereignis, das eine Ausgabe eines Agenten meldet.
pub fn agent_output_event(task: &Task, agent: &str, output: &str, return_code: i64) -> Event {
    let mut payload = Map::new();
    payload.insert("task_id".into(), Value::String(task.task_id.clone()));
    payload.insert("agent".into(), Value::String(agent.to_string()));
    payload.insert("output".into(), Value::String(output.to_string()));
    payload.insert("return_code".into(), Value::Number(return_code.into()));
    build_event(
        task,
        topics::AGENT_THOUGHTS,
        "agent.output",
        &format!("agent.{agent}"),
        payload,
    )
}

/// Baut das Ereignis, das den Beginn einer Ausführung meldet.
pub fn agent_dispatch_start_event(task: &Task, agent: &str) -> Event {
    let mut payload = Map::new();
    payload.insert("task_id".into(), Value::String(task.task_id.clone()));
    payload.insert("agent".into(), Value::String(agent.to_string()));
    build_event(
        task,
        topics::AGENT_HEARTBEAT,
        "agent.dispatch.start",
        "dispatcher",
        payload,
    )
}

/// Baut das Grundgerüst eines Ereignisses.
///
/// Ereigniskennung und Zeitstempel werden hier erzeugt, die Spurkennung kommt
/// aus der Aufgabe. Damit tragen Server, Worker und CLI denselben Vertrag.
fn build_event(
    task: &Task,
    topic: &str,
    event_type: &str,
    producer: &str,
    payload: Map<String, Value>,
) -> Event {
    Event {
        event_id: uuid::Uuid::new_v4().to_string(),
        trace_id: task.trace_id.clone(),
        timestamp: Utc::now().to_rfc3339(),
        topic: topic.to_string(),
        event_type: event_type.to_string(),
        producer: producer.to_string(),
        payload,
        meta: Map::new(),
    }
}

/// Setzt den Archivstatus und den zugehörigen Zeitstempel.
pub fn apply_archived(task: &mut Task, archived: bool) {
    task.archived = archived;
    task.archived_at = if archived {
        Some(Utc::now().to_rfc3339())
    } else {
        None
    };
    task.touch();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uebergang_wird_protokolliert() {
        let mut task = Task::new("JJC-1", "Test");
        apply_transition(&mut task, TaskState::Zhongshu, "taizi", "Weiterleitung").unwrap();
        assert_eq!(task.state, TaskState::Zhongshu);
        assert_eq!(task.prev_state.as_deref(), Some("Taizi"));
        assert_eq!(task.flow_log.len(), 1);
        assert_eq!(task.flow_log[0].from.as_deref(), Some("Taizi"));
    }

    #[test]
    fn unerlaubter_uebergang_aendert_nichts() {
        let mut task = Task::new("JJC-1", "Test");
        let error = apply_transition(&mut task, TaskState::Done, "system", "sofort fertig")
            .expect_err("Direktsprung muss scheitern");
        assert!(matches!(error, AppError::InvalidTransition { .. }));
        assert_eq!(task.state, TaskState::Taizi);
        assert!(task.flow_log.is_empty());
    }

    #[test]
    fn ereignis_der_neuanlage_traegt_den_bekannten_vertrag() {
        let mut task = Task::new("JJC-1", "Test");
        task.assignee_org = Some("尚书省".to_string());
        let event = task_created_event(&task);
        assert_eq!(event.topic, topics::TASK_CREATED);
        assert_eq!(event.event_type, "task.created");
        assert_eq!(event.producer, "task_service");
        assert_eq!(event.trace_id, task.trace_id);
        assert_eq!(event.payload["task_id"], task.task_id);
        assert_eq!(event.payload["title"], "Test");
        assert_eq!(event.payload["state"], "Taizi");
        assert_eq!(event.payload["assignee_org"], "尚书省");
        assert!(uuid::Uuid::parse_str(&event.event_id).is_ok());
    }

    #[test]
    fn endzustand_laeuft_ueber_der_abschluss_thema() {
        let task = Task::new("JJC-1", "Test");
        let event = state_change_event(
            &task,
            TaskState::Review,
            TaskState::Done,
            "geprueft",
            "menxia",
        );
        assert_eq!(event.topic, topics::TASK_COMPLETED);
        assert_eq!(event.event_type, "task.state.Done");
        assert_eq!(event.producer, "menxia");
        assert_eq!(event.payload["from"], "Review");
        assert_eq!(event.payload["to"], "Done");
        assert_eq!(event.payload["reason"], "geprueft");
    }

    #[test]
    fn zwischenzustaende_laufen_ueber_der_status_thema() {
        let task = Task::new("JJC-1", "Test");
        let event = state_change_event(
            &task,
            TaskState::Taizi,
            TaskState::Zhongshu,
            "weiter",
            "taizi",
        );
        assert_eq!(event.topic, topics::TASK_STATUS);
        assert_eq!(event.event_type, "task.state.Zhongshu");
    }

    #[test]
    fn dispatch_und_agent_ereignisse_halten_ihre_felder() {
        let task = Task::new("JJC-1", "Test");
        let dispatch = dispatch_event(&task, "zhongshu", "Bitte pruefen");
        assert_eq!(dispatch.topic, topics::TASK_DISPATCH);
        assert_eq!(dispatch.payload["agent"], "zhongshu");
        assert_eq!(dispatch.payload["message"], "Bitte pruefen");
        assert_eq!(dispatch.payload["state"], "Taizi");

        let start = agent_dispatch_start_event(&task, "zhongshu");
        assert_eq!(start.topic, topics::AGENT_HEARTBEAT);
        assert_eq!(start.event_type, "agent.dispatch.start");
        assert_eq!(start.producer, "dispatcher");

        let output = agent_output_event(&task, "zhongshu", "Ergebnis", 0);
        assert_eq!(output.topic, topics::AGENT_THOUGHTS);
        assert_eq!(output.event_type, "agent.output");
        assert_eq!(output.producer, "agent.zhongshu");
        assert_eq!(output.payload["return_code"], 0);
    }

    #[test]
    fn todo_zustaende_werden_normalisiert() {
        let mut task = Task::new("JJC-1", "Test");
        let todos = vec![TodoItem {
            id: String::new(),
            title: "Schritt".to_string(),
            status: "offen".to_string(),
            detail: None,
        }];
        apply_todos(&mut task, todos);
        assert_eq!(task.todos[0].status, "not-started");
        assert_eq!(task.todos[0].id, "1");
    }
}
