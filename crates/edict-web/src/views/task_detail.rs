//! Detailansicht einer Aufgabe.

use dioxus::prelude::*;

use crate::app::{Action, Command, UiContext, View};
use crate::components::status_badge::{PriorityBadge, StateBadge};
use crate::models::{
    dispatch_status_label, format_timestamp, heartbeat_label, official_label, org_label,
    relative_time, server_state_label, todo_status_label, ActivityEntry, FlowEntry,
    SchedulerStateData, TaskActivityData, TaskState, TodoItem, TODO_STATUSES,
};

/// Der naechste Zustand im Ablauf, wie ihn die Oberflaeche beim Fortschalten nennt.
fn next_state_label(state: TaskState) -> &'static str {
    match state {
        TaskState::Pending => "Taizi",
        TaskState::Taizi => "Zhongshu",
        TaskState::Zhongshu => "Menxia",
        TaskState::Menxia => "Assigned",
        TaskState::Assigned => "Doing",
        TaskState::Next => "Doing",
        TaskState::Doing => "Review",
        TaskState::Review => "Done",
        TaskState::Blocked => "Doing",
        TaskState::Done => "Done",
        TaskState::Cancelled => "Cancelled",
    }
}

/// Formt einen Aktivitaetseintrag in eine Zeile.
fn activity_line(entry: &ActivityEntry) -> String {
    let text = entry
        .text
        .clone()
        .or_else(|| entry.remark.clone())
        .or_else(|| entry.thinking.clone())
        .or_else(|| entry.output.clone())
        .unwrap_or_else(|| entry.kind.clone());
    match entry.agent.clone() {
        Some(agent) if !agent.is_empty() => format!("{agent}: {text}"),
        _ => text,
    }
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

/// Aendert ein Feld einer Teilaufgabe in der Liste.
fn change_todo(mut todos: Signal<Vec<TodoItem>>, id: &str, change: impl FnOnce(&mut TodoItem)) {
    let mut list = todos.write();
    if let Some(todo) = list.iter_mut().find(|todo| todo.id == id) {
        change(todo);
    }
}

/// Die Auswahlwerte fuer den Status einer Teilaufgabe.
fn todo_status_options() -> Vec<(String, &'static str)> {
    TODO_STATUSES
        .iter()
        .map(|status| (status.to_string(), todo_status_label(status)))
        .collect()
}

/// Eine Teilaufgabe in der Liste mit Titel, Status und Entfernen-Schaltflaeche.
fn todo_row(todos: Signal<Vec<TodoItem>>, todo: &TodoItem) -> Element {
    let id = todo.id.clone();
    let title = todo.title.clone();
    let status = todo.status.clone();
    let options = todo_status_options();

    let title_id = id.clone();
    let status_id = id.clone();
    let remove_id = id.clone();

    rsx! {
        li { class: "todo-item", key: "{id}",
            input {
                r#type: "text",
                class: "todo-title",
                value: "{title}",
                aria_label: "Titel der Teilaufgabe",
                oninput: move |event| {
                    let value = event.value();
                    change_todo(todos, &title_id, |todo| { todo.title = value; });
                },
            },
            select {
                class: "todo-status",
                value: "{status}",
                aria_label: "Status der Teilaufgabe",
                onchange: move |event| {
                    let value = event.value();
                    change_todo(todos, &status_id, |todo| { todo.status = value; });
                },
                for (value, label) in options.iter() {
                    option { value: "{value}", "{label}" }
                }
            },
            button {
                class: "toast-close",
                r#type: "button",
                aria_label: "Teilaufgabe entfernen",
                onclick: move |_| {
                    let mut todos = todos;
                    let mut list = todos.write();
                    list.retain(|todo| todo.id != remove_id);
                },
                "×"
            }
        }
    }
}

/// Ein Eintrag des Zustandsprotokolls.
fn flow_row(entry: &FlowEntry) -> Element {
    let at = entry.at.clone().unwrap_or_default();
    let transition = format!(
        "{} → {}",
        org_label(&entry.from.clone().unwrap_or_default()),
        org_label(&entry.to.clone().unwrap_or_default())
    );
    let remark = entry.remark.clone();

    rsx! {
        li { class: "log-item",
            div { class: "log-time", {at} }
            div { {transition} }
            if let Some(remark) = remark {
                div { {remark} }
            }
        }
    }
}

/// Ein Eintrag der Aktivitaetsliste.
fn activity_row(entry: &ActivityEntry) -> Element {
    let time = activity_time(entry);
    let text = activity_line(entry);

    rsx! {
        li { class: "log-item",
            div { class: "log-time", {time} }
            div { {text} }
        }
    }
}

/// Die Scheduler-Werte der gewaehlten Aufgabe.
fn scheduler_panel(data: Option<SchedulerStateData>) -> Element {
    let Some(data) = data else {
        return rsx! { p { "Die Scheduler-Daten wurden noch nicht geladen." } };
    };
    let Some(scheduler) = data.scheduler.clone() else {
        return rsx! { p { "Fuer diese Aufgabe liegen keine Scheduler-Daten vor." } };
    };

    rsx! {
        dl { class: "definition-list",
            dt { "Versuche" }
            dd { "{scheduler.retry_count.unwrap_or(0)}" }
            dt { "Eskalationsstufe" }
            dd { "{scheduler.escalation_level.unwrap_or(0)}" }
            dt { "Letzter Versand" }
            dd { {dispatch_status_label(&scheduler.last_dispatch_status.clone().unwrap_or_default())} }
            dt { "Letzter Agent" }
            dd { "{scheduler.last_dispatch_agent.clone().unwrap_or_default()}" }
            dt { "Letzter Fortschritt" }
            dd { {format_timestamp(&scheduler.last_progress_at.clone().unwrap_or_default())} }
            dt { "Stillstand" }
            dd { "{data.stalled_sec.unwrap_or(0)} Sekunden" }
        }
    }
}

/// Die Aktivitaet der gewaehlten Aufgabe.
fn activity_panel(data: Option<TaskActivityData>) -> Element {
    let Some(data) = data else {
        return rsx! { p { "Die Aktivitaet wurde noch nicht geladen." } };
    };

    rsx! {
        if data.activity.is_empty() {
            p { "Zu dieser Aufgabe wurde noch keine Aktivitaet aufgezeichnet." }
        } else {
            ul { class: "log-list",
                for entry in data.activity.iter().rev() {
                    {activity_row(entry)}
                }
            }
        }
        if let Some(label) = data.agent_label.clone() {
            p { class: "task-card-line", "Zustaendiger Agent: {server_state_label(&label)}" }
        }
    }
}

/// Detailansicht mit allen Feldern und Aktionen der gewaehlten Aufgabe.
#[component]
pub fn TaskDetail() -> Element {
    let context = use_context::<UiContext>();
    let state = context.state;

    let selected_id = state.selected_task.read().clone();
    let tasks = state.tasks.read().clone();
    let task = tasks
        .iter()
        .find(|task| task.display_id() == selected_id)
        .cloned();

    let mut view = state.view;
    let mut comment = use_signal(String::new);
    let mut reason = use_signal(String::new);
    let mut review_comment = use_signal(String::new);
    let mut wake_message = use_signal(String::new);
    let mut new_todo_title = use_signal(String::new);
    let mut todos = use_signal(Vec::<TodoItem>::new);
    let mut loaded_todos_for = use_signal(String::new);

    // Die Teilaufgaben werden nur beim Wechsel der Aufgabe vom Server uebernommen.
    // Sonst wuerde die Abfrage alle fuenf Sekunden offene Aenderungen verwerfen.
    use_effect(move || {
        let current = state.selected_task.read().clone();
        let found = state
            .tasks
            .read()
            .iter()
            .find(|task| task.display_id() == current)
            .cloned();
        let Some(found) = found else {
            return;
        };
        if *loaded_todos_for.peek() == current {
            return;
        }
        loaded_todos_for.set(current);
        todos.set(found.todos.clone());
    });

    let back = move |_| view.set(View::Board);

    let Some(task) = task else {
        return rsx! {
            section { class: "detail-view",
                button { class: "back-link", r#type: "button", onclick: back, "Zurueck zum Board" }
                div { class: "state-message", "Die Aufgabe wurde nicht gefunden." }
            }
        };
    };

    let task_id = task.display_id().to_string();
    let can_stop = !matches!(
        task.state,
        TaskState::Done | TaskState::Blocked | TaskState::Cancelled
    );
    let can_resume = matches!(task.state, TaskState::Blocked | TaskState::Cancelled);
    let responsible = task.responsible_agent().unwrap_or_default().to_string();

    let advance = {
        let commands = context.commands.clone();
        let task_id = task_id.clone();
        move |_| {
            let mut comment = comment;
            let text = comment.read().trim().to_string();
            comment.set(String::new());
            let _ = commands.unbounded_send(Command::Action(Action::AdvanceState {
                task_id: task_id.clone(),
                comment: text,
            }));
        }
    };
    let archive = {
        let commands = context.commands.clone();
        let task_id = task_id.clone();
        let archived = !task.archived;
        move |_| {
            let _ = commands.unbounded_send(Command::Action(Action::Archive {
                task_id: task_id.clone(),
                archived,
            }));
        }
    };
    let approve = {
        let commands = context.commands.clone();
        let task_id = task_id.clone();
        move |_| {
            let mut review_comment = review_comment;
            let text = review_comment.read().trim().to_string();
            review_comment.set(String::new());
            let _ = commands.unbounded_send(Command::Action(Action::Review {
                task_id: task_id.clone(),
                action: "approve".to_string(),
                comment: text,
            }));
        }
    };
    let reject = {
        let commands = context.commands.clone();
        let task_id = task_id.clone();
        move |_| {
            let mut review_comment = review_comment;
            let text = review_comment.read().trim().to_string();
            review_comment.set(String::new());
            let _ = commands.unbounded_send(Command::Action(Action::Review {
                task_id: task_id.clone(),
                action: "reject".to_string(),
                comment: text,
            }));
        }
    };
    let stop = {
        let commands = context.commands.clone();
        let task_id = task_id.clone();
        move |_| {
            let mut reason = reason;
            let text = reason.read().trim().to_string();
            reason.set(String::new());
            let _ = commands.unbounded_send(Command::Action(Action::TaskControl {
                task_id: task_id.clone(),
                action: "stop".to_string(),
                reason: text,
            }));
        }
    };
    let cancel = {
        let commands = context.commands.clone();
        let task_id = task_id.clone();
        move |_| {
            let mut reason = reason;
            let text = reason.read().trim().to_string();
            reason.set(String::new());
            let _ = commands.unbounded_send(Command::Action(Action::TaskControl {
                task_id: task_id.clone(),
                action: "cancel".to_string(),
                reason: text,
            }));
        }
    };
    let resume = {
        let commands = context.commands.clone();
        let task_id = task_id.clone();
        move |_| {
            let mut reason = reason;
            let text = reason.read().trim().to_string();
            reason.set(String::new());
            let _ = commands.unbounded_send(Command::Action(Action::TaskControl {
                task_id: task_id.clone(),
                action: "resume".to_string(),
                reason: text,
            }));
        }
    };
    let wake = {
        let commands = context.commands.clone();
        let agent_id = responsible.clone();
        move |_| {
            let mut wake_message = wake_message;
            let text = wake_message.read().trim().to_string();
            wake_message.set(String::new());
            let _ = commands.unbounded_send(Command::Action(Action::Wake {
                agent_id: agent_id.clone(),
                message: text,
            }));
        }
    };
    let save_todos = {
        let commands = context.commands.clone();
        let task_id = task_id.clone();
        move |_| {
            let list = todos.read().clone();
            let _ = commands.unbounded_send(Command::Action(Action::UpdateTodos {
                task_id: task_id.clone(),
                todos: list,
            }));
        }
    };
    let add_todo = move |_| {
        let mut new_todo_title = new_todo_title;
        let title = new_todo_title.read().trim().to_string();
        if title.is_empty() {
            return;
        }
        new_todo_title.set(String::new());
        let mut todos = todos;
        let mut list = todos.write();
        let next_id = list.len() + 1;
        list.push(TodoItem {
            id: next_id.to_string(),
            title,
            status: "not-started".to_string(),
            detail: None,
        });
    };

    let todo_list = todos.read().clone();
    let activity = state.detail_activity.read().clone();
    let scheduler = state.detail_scheduler.read().clone();

    rsx! {
        section { class: "detail-view",
            button { class: "back-link", r#type: "button", onclick: back, "Zurueck zum Board" }
            div { class: "detail-layout",
                div { class: "detail-main",
                    div { class: "panel",
                        h2 { class: "detail-title", {task.title.clone()} }
                        div { class: "detail-meta",
                            StateBadge { state: task.state }
                            if !task.priority.is_empty() {
                                PriorityBadge { priority: task.priority.clone() }
                            }
                            span { class: "badge", {org_label(&task.org)} }
                            span { class: "badge", {task.state_text()} }
                            if task.archived {
                                span { class: "badge badge-warning", "Archiviert" }
                            }
                        }
                        dl { class: "definition-list",
                            dt { "Kennung" }
                            dd { class: "task-card-id", {task_id.clone()} }
                            if !task.trace_id.is_empty() {
                                dt { "Trace" }
                                dd { class: "task-card-id", {task.trace_id.clone()} }
                            }
                            dt { "Zustand" }
                            dd { {task.state.as_str()} }
                            if !task.official.is_empty() {
                                dt { "Zustaendig" }
                                dd { {official_label(&task.official)} }
                            }
                            dt { "Organisation" }
                            dd { {org_label(&task.org)} }
                            if !task.now.is_empty() {
                                dt { "Aktueller Stand" }
                                dd { {task.now.clone()} }
                            }
                            if !task.eta.is_empty() {
                                dt { "Termin" }
                                dd { {task.eta.clone()} }
                            }
                            if !task.block.is_empty() {
                                dt { "Blockiert durch" }
                                dd { {task.block.clone()} }
                            }
                            if !task.ac.is_empty() {
                                dt { "Abnahmekriterien" }
                                dd { {task.ac.clone()} }
                            }
                            if !task.output.is_empty() {
                                dt { "Ergebnis" }
                                dd { {task.output.clone()} }
                            }
                            if !task.description.is_empty() {
                                dt { "Beschreibung" }
                                dd { {task.description.clone()} }
                            }
                            dt { "Pruefrunde" }
                            dd { "{task.review_round}" }
                            if !task.created_at.is_empty() {
                                dt { "Angelegt" }
                                dd { {format_timestamp(&task.created_at)} }
                            }
                            if !task.updated_at.is_empty() {
                                dt { "Aktualisiert" }
                                dd { {relative_time(&task.updated_at)} }
                            }
                            if let Some(archived_at) = task.archived_at.clone() {
                                dt { "Archiviert am" }
                                dd { {format_timestamp(&archived_at)} }
                            }
                            if let Some(heartbeat) = task.heartbeat.clone() {
                                dt { "Herzschlag" }
                                dd { {heartbeat_label(&heartbeat)} }
                            }
                            if let Some(meta) = task.source_meta.clone() {
                                dt { "Herkunft" }
                                dd { {serde_json::to_string(&meta).unwrap_or_default()} }
                            }
                        }
                    }

                    div { class: "panel",
                        h3 { "Teilaufgaben" }
                        if todo_list.is_empty() {
                            p { "Diese Aufgabe hat keine Teilaufgaben." }
                        } else {
                            ul { class: "todo-list",
                                for todo in todo_list.iter() {
                                    {todo_row(todos, todo)}
                                }
                            }
                        }
                        div { class: "action-row",
                            input {
                                r#type: "text",
                                placeholder: "Neue Teilaufgabe",
                                aria_label: "Neue Teilaufgabe",
                                value: "{new_todo_title.read()}",
                                oninput: move |event| new_todo_title.set(event.value()),
                            },
                            button {
                                class: "secondary-button",
                                r#type: "button",
                                onclick: add_todo,
                                "Teilaufgabe hinzufuegen"
                            },
                            button {
                                class: "primary-button",
                                r#type: "button",
                                onclick: save_todos,
                                "Teilaufgaben speichern"
                            }
                        }
                    }

                    div { class: "panel",
                        h3 { "Zustandsprotokoll" }
                        if task.flow_log.is_empty() {
                            p { "Zu dieser Aufgabe liegt kein Zustandsprotokoll vor." }
                        } else {
                            ul { class: "log-list",
                                for entry in task.flow_log.iter().rev() {
                                    {flow_row(entry)}
                                }
                            }
                        }
                    }
                }

                div { class: "detail-side",
                    div { class: "panel",
                        h3 { "Zustand weiterschalten" }
                        p { "Der naechste Schritt ist: {next_state_label(task.state)}." }
                        div { class: "action-row",
                            input {
                                r#type: "text",
                                placeholder: "Kommentar zum Uebergang",
                                aria_label: "Kommentar zum Uebergang",
                                value: "{comment.read()}",
                                oninput: move |event| comment.set(event.value()),
                            },
                            button {
                                class: "primary-button",
                                r#type: "button",
                                onclick: advance,
                                "Weiterschalten"
                            }
                        }
                    }

                    div { class: "panel",
                        h3 { "Archivierung" }
                        p {
                            if task.archived {
                                "Die Aufgabe liegt im Archiv."
                            } else {
                                "Die Aufgabe liegt nicht im Archiv."
                            }
                        }
                        div { class: "action-row",
                            button {
                                class: if task.archived { "secondary-button" } else { "danger-button" },
                                r#type: "button",
                                onclick: archive,
                                if task.archived { "Wiederherstellen" } else { "Archivieren" }
                            }
                        }
                    }

                    div { class: "panel",
                        h3 { "Ablauf steuern" }
                        div { class: "action-row",
                            input {
                                r#type: "text",
                                placeholder: "Begruendung",
                                aria_label: "Begruendung der Aktion",
                                value: "{reason.read()}",
                                oninput: move |event| reason.set(event.value()),
                            }
                        }
                        div { class: "action-row",
                            if can_stop {
                                button {
                                    class: "secondary-button",
                                    r#type: "button",
                                    onclick: stop,
                                    "Anhalten"
                                }
                                button {
                                    class: "danger-button",
                                    r#type: "button",
                                    onclick: cancel,
                                    "Abbrechen"
                                }
                            }
                            if can_resume {
                                button {
                                    class: "secondary-button",
                                    r#type: "button",
                                    onclick: resume,
                                    "Fortsetzen"
                                }
                            }
                        }
                    }

                    div { class: "panel",
                        h3 { "Abnahme" }
                        div { class: "action-row",
                            input {
                                r#type: "text",
                                placeholder: "Kommentar zur Abnahme",
                                aria_label: "Kommentar zur Abnahme",
                                value: "{review_comment.read()}",
                                oninput: move |event| review_comment.set(event.value()),
                            }
                        }
                        div { class: "action-row",
                            button {
                                class: "secondary-button",
                                r#type: "button",
                                onclick: approve,
                                "Genehmigen"
                            },
                            button {
                                class: "danger-button",
                                r#type: "button",
                                onclick: reject,
                                "Zurueckweisen"
                            }
                        }
                    }

                    if !responsible.is_empty() {
                        div { class: "panel",
                            h3 { "Agent wecken" }
                            p { "Zustaendiger Agent: {responsible}" }
                            div { class: "action-row",
                                input {
                                    r#type: "text",
                                    placeholder: "Nachricht an den Agenten",
                                    aria_label: "Nachricht an den Agenten",
                                    value: "{wake_message.read()}",
                                    oninput: move |event| wake_message.set(event.value()),
                                },
                                button {
                                    class: "primary-button",
                                    r#type: "button",
                                    onclick: wake,
                                    "Aufwecken"
                                }
                            }
                        }
                    }

                    div { class: "panel",
                        h3 { "Scheduler" }
                        {scheduler_panel(scheduler)}
                    }

                    div { class: "panel",
                        h3 { "Aktivitaet" }
                        {activity_panel(activity)}
                    }
                }
            }
        }
    }
}
