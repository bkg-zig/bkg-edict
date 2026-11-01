//! Filter und Sortierung der Aufgabenliste.

use dioxus::prelude::*;

use crate::app::{ArchiveFilter, SortOrder, UiContext};
use crate::models::{org_label, state_label, TaskState, BOARD_COLUMNS};

/// Filterleiste ueber dem Board.
#[component]
pub fn FilterBar() -> Element {
    let context = use_context::<UiContext>();
    let mut filter = context.state.filter;
    let current = filter.read().clone();

    let mut organisations: Vec<String> = context
        .state
        .tasks
        .read()
        .iter()
        .map(|task| task.org.clone())
        .filter(|org| !org.is_empty())
        .collect();
    organisations.sort();
    organisations.dedup();

    let on_state = move |event: Event<FormData>| {
        let value = event.value();
        let mut current = filter.read().clone();
        current.state = TaskState::parse(&value);
        filter.set(current);
    };
    let on_org = move |event: Event<FormData>| {
        let value = event.value();
        let mut current = filter.read().clone();
        current.org = value;
        filter.set(current);
    };
    let on_text = move |event: Event<FormData>| {
        let value = event.value();
        let mut current = filter.read().clone();
        current.text = value;
        filter.set(current);
    };
    let on_archive = move |event: Event<FormData>| {
        let value = event.value();
        let mut current = filter.read().clone();
        current.archive = ArchiveFilter::ALL
            .iter()
            .copied()
            .find(|item| item.label() == value)
            .unwrap_or(ArchiveFilter::All);
        filter.set(current);
    };
    let on_sort = move |event: Event<FormData>| {
        let value = event.value();
        let mut current = filter.read().clone();
        current.sort = SortOrder::ALL
            .iter()
            .copied()
            .find(|item| item.label() == value)
            .unwrap_or(SortOrder::Updated);
        filter.set(current);
    };
    let reset = move |_| filter.set(crate::app::Filter::default());

    rsx! {
        div { class: "filter-bar", role: "search", aria_label: "Aufgaben filtern",
            label { class: "field",
                span { "Zustand" }
                select {
                    value: current.state.map(|state| state.as_str()).unwrap_or_default(),
                    onchange: on_state,
                    option { value: "", "Alle Zustaende" }
                    for state in BOARD_COLUMNS.iter() {
                        option { value: state.as_str(), {state_label(*state)} }
                    }
                }
            }
            label { class: "field",
                span { "Organisation" }
                select {
                    value: current.org.clone(),
                    onchange: on_org,
                    option { value: "", "Alle Organisationen" }
                    for org in organisations.iter() {
                        option { value: org.clone(), {org_label(org)} }
                    }
                }
            }
            label { class: "field field-grow",
                span { "Titel" }
                input {
                    r#type: "search",
                    value: current.text.clone(),
                    placeholder: "Titel durchsuchen",
                    aria_label: "Titel durchsuchen",
                    oninput: on_text,
                }
            }
            label { class: "field",
                span { "Umfang" }
                select {
                    value: current.archive.label(),
                    onchange: on_archive,
                    for option in ArchiveFilter::ALL.iter() {
                        option { value: option.label(), {option.label()} }
                    }
                }
            }
            label { class: "field",
                span { "Sortierung" }
                select {
                    value: current.sort.label(),
                    onchange: on_sort,
                    for option in SortOrder::ALL.iter() {
                        option { value: option.label(), {option.label()} }
                    }
                }
            }
            button {
                class: "secondary-button",
                r#type: "button",
                onclick: reset,
                "Filter zuruecksetzen"
            }
        }
    }
}
