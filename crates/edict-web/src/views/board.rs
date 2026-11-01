//! Board mit den Aufgaben nach Zustand.

use dioxus::prelude::*;

use crate::app::{Action, ArchiveFilter, Command, SortOrder, UiContext};
use crate::components::filter_bar::FilterBar;
use crate::components::task_card::TaskCard;
use crate::models::{Task, TaskState, BOARD_COLUMNS};

/// Prueft, ob eine Aufgabe zum aktuellen Filter passt.
fn matches_filter(task: &Task, state: Option<TaskState>, org: &str, text: &str) -> bool {
    if let Some(wanted) = state {
        if task.state != wanted {
            return false;
        }
    }
    if !org.is_empty() && task.org != org {
        return false;
    }
    if !text.is_empty() && !task.title.to_lowercase().contains(&text.to_lowercase()) {
        return false;
    }
    true
}

/// Sortiert die Aufgabenliste nach der gewaehlten Reihenfolge.
fn sort_tasks(mut tasks: Vec<Task>, order: SortOrder) -> Vec<Task> {
    match order {
        SortOrder::Updated => tasks.sort_by(|left, right| right.updated_at.cmp(&left.updated_at)),
        SortOrder::Title => tasks.sort_by(|left, right| left.title.cmp(&right.title)),
    }
    tasks
}

/// Das Board mit allen Spalten.
#[component]
pub fn Board() -> Element {
    let context = use_context::<UiContext>();
    let state = context.state;

    let filter = state.filter.read().clone();
    let all_tasks = state.tasks.read().clone();
    let loading = *state.loading.read();
    let error = state.error.read().clone();

    let visible = sort_tasks(
        all_tasks
            .into_iter()
            .filter(|task| match filter.archive {
                ArchiveFilter::All => true,
                ArchiveFilter::Open => !task.is_archived(),
                ArchiveFilter::Archived => task.is_archived(),
            })
            .filter(|task| {
                matches_filter(
                    task,
                    filter.state,
                    filter.org.as_str(),
                    filter.text.as_str(),
                )
            })
            .collect(),
        filter.sort,
    );

    let archive_all = {
        let commands = context.commands.clone();
        move |_| {
            let _ = commands.unbounded_send(Command::Action(Action::ArchiveAllDone));
        }
    };
    let reload = {
        let commands = context.commands.clone();
        move |_| {
            let _ = commands.unbounded_send(Command::ReloadTasks);
        }
    };

    let columns: Vec<(TaskState, Vec<Task>)> = BOARD_COLUMNS
        .iter()
        .map(|column| {
            let column_tasks = visible
                .iter()
                .filter(|task| task.state == *column)
                .cloned()
                .collect();
            (*column, column_tasks)
        })
        .collect();

    rsx! {
        section { class: "board-view",
            FilterBar {}
            div { class: "action-row",
                button {
                    class: "secondary-button",
                    r#type: "button",
                    onclick: reload,
                    "Neu laden"
                }
                button {
                    class: "secondary-button",
                    r#type: "button",
                    onclick: archive_all,
                    "Alle abgeschlossenen Aufgaben archivieren"
                }
                span { class: "task-card-line", "{visible.len()} Aufgaben werden angezeigt" }
            }
            if let Some(message) = error {
                div { class: "state-message state-message-error", role: "alert",
                    "Die Aufgabenliste konnte nicht geladen werden. {message}"
                }
            } else if loading && visible.is_empty() {
                div { class: "state-message", "Die Aufgabenliste wird geladen." }
            } else if visible.is_empty() {
                div { class: "state-message",
                    "Keine Aufgabe passt zum Filter. Aendern Sie die Filter oder legen Sie eine neue Aufgabe an."
                }
            } else {
                div { class: "board", role: "list", aria_label: "Aufgaben nach Zustand",
                    for (column, column_tasks) in columns.iter() {
                        div { class: "board-column", role: "listitem",
                            div { class: "board-column-header",
                                span { {column.label()} }
                                span { class: "board-column-count", "{column_tasks.len()}" }
                            }
                            for task in column_tasks.iter() {
                                TaskCard { task: task.clone() }
                            }
                        }
                    }
                }
            }
        }
    }
}
