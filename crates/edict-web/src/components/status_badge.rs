//! Beschriftungen fuer Zustaende und Prioritaeten.

use dioxus::prelude::*;

use crate::models::{priority_label, state_label, TaskState};

/// CSS-Klasse fuer einen Zustand.
fn state_class(state: TaskState) -> &'static str {
    match state {
        TaskState::Doing | TaskState::Assigned => "badge badge-accent",
        TaskState::Review => "badge badge-warning",
        TaskState::Done => "badge badge-success",
        TaskState::Blocked | TaskState::Cancelled => "badge badge-danger",
        _ => "badge",
    }
}

/// Badge mit dem Zustand einer Aufgabe.
#[component]
pub fn StateBadge(state: TaskState) -> Element {
    rsx! {
        span { class: state_class(state), {state_label(state)} }
    }
}

/// Badge mit der Prioritaet einer Aufgabe.
#[component]
pub fn PriorityBadge(priority: String) -> Element {
    let class = match priority.trim().to_ascii_lowercase().as_str() {
        "high" | "hoch" | "高" | "紧急" => "badge badge-danger",
        "urgent" | "dringend" => "badge badge-danger",
        "low" | "niedrig" | "低" => "badge",
        _ => "badge badge-accent",
    };
    rsx! {
        span { class: class, {priority_label(&priority)} }
    }
}
