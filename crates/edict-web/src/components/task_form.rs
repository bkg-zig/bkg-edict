//! Dialog zum Anlegen einer Aufgabe.

use dioxus::prelude::*;

use crate::app::{push_toast, Action, Command, ToastKind, UiContext};
use crate::models::NewTask;

/// Prioritaeten, die der Server versteht.
const PRIORITIES: [(&str, &str); 4] = [
    ("低", "Niedrig"),
    ("中", "Mittel"),
    ("高", "Hoch"),
    ("紧急", "Dringend"),
];

/// Ministerien, an die eine Aufgabe gestellt werden kann.
const TARGET_DEPTS: [(&str, &str); 6] = [
    ("礼部", "Rites-Ministerium"),
    ("兵部", "Kriegsministerium"),
    ("户部", "Finanzministerium"),
    ("刑部", "Justizministerium"),
    ("工部", "Bauministerium"),
    ("吏部", "Personalministerium"),
];

/// Dialog zum Anlegen einer Aufgabe.
#[component]
pub fn CreateTaskDialog() -> Element {
    let context = use_context::<UiContext>();
    let mut open = context.state.create_task_open;
    let mut title = use_signal(String::new);
    let mut description = use_signal(String::new);
    let mut priority = use_signal(|| "中".to_string());
    let mut target = use_signal(|| "工部".to_string());

    let submit = move |_| {
        let task = NewTask {
            title: title.read().trim().to_string(),
            description: description.read().trim().to_string(),
            org: String::new(),
            priority: priority.read().clone(),
            target_dept: target.read().clone(),
        };
        if task.title.is_empty() {
            push_toast(
                context.state,
                ToastKind::Error,
                "Der Titel der Aufgabe fehlt.",
            );
            return;
        }
        if context
            .commands
            .unbounded_send(Command::Action(Action::CreateTask(task)))
            .is_err()
        {
            push_toast(
                context.state,
                ToastKind::Error,
                "Die Aufgabe konnte nicht gesendet werden.",
            );
            return;
        }
        title.set(String::new());
        description.set(String::new());
        open.set(false);
    };

    rsx! {
        if *open.read() {
            div {
                class: "modal-backdrop",
                role: "presentation",
                onclick: move |_| open.set(false),
                div {
                    class: "modal",
                    role: "dialog",
                    aria_modal: "true",
                    aria_label: "Neue Aufgabe anlegen",
                    onclick: move |event| event.stop_propagation(),
                    h2 { "Neue Aufgabe anlegen" }
                    label { class: "field",
                        span { "Titel" }
                        input {
                            r#type: "text",
                            value: "{title.read()}",
                            placeholder: "Worum geht es?",
                            aria_label: "Titel der Aufgabe",
                            oninput: move |event| title.set(event.value()),
                        }
                    }
                    label { class: "field",
                        span { "Beschreibung" }
                        textarea {
                            value: "{description.read()}",
                            placeholder: "Ergaenzende Angaben zur Aufgabe",
                            aria_label: "Beschreibung der Aufgabe",
                            oninput: move |event| description.set(event.value()),
                        }
                    }
                    label { class: "field",
                        span { "Prioritaet" }
                        select {
                            value: "{priority.read()}",
                            aria_label: "Prioritaet der Aufgabe",
                            onchange: move |event| priority.set(event.value()),
                            for (value, label) in PRIORITIES.iter() {
                                option { value: *value, {*label} }
                            }
                        }
                    }
                    label { class: "field",
                        span { "Zustellung" }
                        select {
                            value: "{target.read()}",
                            aria_label: "Zustellung der Aufgabe",
                            onchange: move |event| target.set(event.value()),
                            for (value, label) in TARGET_DEPTS.iter() {
                                option { value: *value, {*label} }
                            }
                        }
                    }
                    div { class: "modal-actions",
                        button {
                            class: "secondary-button",
                            r#type: "button",
                            onclick: move |_| open.set(false),
                            "Abbrechen"
                        }
                        button {
                            class: "primary-button",
                            r#type: "button",
                            onclick: submit,
                            "Aufgabe anlegen"
                        }
                    }
                }
            }
        }
    }
}
