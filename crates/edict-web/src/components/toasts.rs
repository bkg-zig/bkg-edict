//! Meldungen am unteren Rand der Oberflaeche.

use dioxus::prelude::*;

use crate::app::{Toast, UiContext};

/// Eine Meldung mit Text und Schaltflaeche zum Schliessen.
fn toast_entry(toasts: Signal<Vec<Toast>>, toast: &Toast) -> Element {
    let toast_id = toast.id;
    let text = toast.text.clone();
    let class = match toast.kind {
        crate::app::ToastKind::Success => "toast toast-success",
        crate::app::ToastKind::Error => "toast toast-error",
        crate::app::ToastKind::Info => "toast toast-info",
    };

    rsx! {
        div {
            class: "{class}",
            span { "{text}" }
            button {
                class: "toast-close",
                r#type: "button",
                aria_label: "Meldung schliessen",
                onclick: move |_| {
                    let mut toasts = toasts;
                    let mut list = toasts.write();
                    list.retain(|item| item.id != toast_id);
                },
                "×"
            }
        }
    }
}

/// Die Meldungen am unteren Rand.
#[component]
pub fn ToastStack() -> Element {
    let context = use_context::<UiContext>();
    let toasts = context.state.toasts;
    let entries = toasts.read().clone();

    rsx! {
        div { class: "toast-stack", role: "status", aria_live: "polite",
            for toast in entries.iter() {
                {toast_entry(toasts, toast)}
            }
        }
    }
}
