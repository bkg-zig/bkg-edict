//! Karte einer Aufgabe im Board.

use dioxus::prelude::*;

use crate::app::{Command, UiContext, View};
use crate::components::status_badge::{PriorityBadge, StateBadge};
use crate::models::{official_label, org_label, pipeline_index, relative_time, Task, PIPELINE};

/// Karte einer Aufgabe mit Titel, Zustand, Organisation und Fortschritt.
#[component]
pub fn TaskCard(task: Task) -> Element {
    let context = use_context::<UiContext>();
    let mut selected_task = context.state.selected_task;
    let mut view = context.state.view;
    let commands = context.commands.clone();

    let id = task.display_id().to_string();
    let title = task.title.clone();
    let state = task.state;
    let priority = task.priority.clone();
    let org = org_label(&task.org);
    let official = official_label(&task.official);
    let now = task.now.clone();
    let block = task.block.clone();
    let eta = task.eta.clone();
    let progress = task.todo_progress();
    let step = pipeline_index(state);
    let updated = if task.updated_at.is_empty() {
        String::new()
    } else {
        relative_time(&task.updated_at)
    };

    rsx! {
        button {
            class: "task-card",
            r#type: "button",
            aria_label: "Aufgabe {title} oeffnen",
            onclick: move |_| {
                selected_task.set(id.clone());
                view.set(View::Detail);
                let _ = commands.unbounded_send(Command::LoadDetail(id.clone()));
            },
            div { class: "task-card-title", {title.clone()} }
            div { class: "task-card-meta",
                StateBadge { state }
                if !priority.is_empty() {
                    PriorityBadge { priority: priority.clone() }
                }
                span { class: "badge", {org.clone()} }
            }
            div { class: "task-card-id", {id.clone()} }
            if !now.is_empty() {
                div { class: "task-card-line", {now.clone()} }
            }
            if !block.is_empty() && block != "Keine" {
                div { class: "task-card-line", "Blockiert: {block}" }
            }
            if !eta.is_empty() {
                div { class: "task-card-line", "Termin: {eta}" }
            }
            if !official.is_empty() {
                div { class: "task-card-line", {official.clone()} }
            }
            if !updated.is_empty() {
                div { class: "task-card-line", "Aktualisiert {updated}" }
            }
            if !task.todos.is_empty() {
                div { class: "task-card-line", "Teilaufgaben: {progress} Prozent" }
                div { class: "progress",
                    div { class: "progress-bar", style: "width: {progress}%" }
                }
            }
            div { class: "pipeline", aria_label: "Fortschritt im Ablauf",
                for (index, _station) in PIPELINE.iter().enumerate() {
                    span {
                        class: if index < step {
                            "pipeline-step pipeline-step-done"
                        } else if index == step {
                            "pipeline-step pipeline-step-current"
                        } else {
                            "pipeline-step"
                        },
                    }
                }
            }
        }
    }
}
