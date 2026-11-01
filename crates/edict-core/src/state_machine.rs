//! Zustandsmaschine der三省六部-Aufgabenverwaltung.
//!
//! Die Übergänge entsprechen der bisherigen Python-Implementierung in
//! `edict/backend/app/models/task.py`.

use std::collections::HashSet;

use crate::model::TaskState;

/// Zustände, in denen eine Aufgabe abgeschlossen ist.
pub const TERMINAL_STATES: [TaskState; 2] = [TaskState::Done, TaskState::Cancelled];

/// Erlaubte Zustandsübergänge.
pub const STATE_TRANSITIONS: &[(TaskState, &[TaskState])] = &[
    (
        TaskState::Taizi,
        &[TaskState::Zhongshu, TaskState::Cancelled],
    ),
    (
        TaskState::Zhongshu,
        &[TaskState::Menxia, TaskState::Cancelled, TaskState::Blocked],
    ),
    (
        TaskState::Menxia,
        &[
            TaskState::Assigned,
            TaskState::Zhongshu,
            TaskState::Cancelled,
        ],
    ),
    (
        TaskState::Assigned,
        &[
            TaskState::Doing,
            TaskState::Next,
            TaskState::Cancelled,
            TaskState::Blocked,
        ],
    ),
    (TaskState::Next, &[TaskState::Doing, TaskState::Cancelled]),
    (
        TaskState::Doing,
        &[
            TaskState::Review,
            TaskState::Done,
            TaskState::Blocked,
            TaskState::Cancelled,
        ],
    ),
    (
        TaskState::Review,
        &[TaskState::Done, TaskState::Doing, TaskState::Cancelled],
    ),
    (
        TaskState::Blocked,
        &[
            TaskState::Taizi,
            TaskState::Zhongshu,
            TaskState::Menxia,
            TaskState::Assigned,
            TaskState::Doing,
        ],
    ),
];

/// Zustand, aus dem die Aufgabe direkt in einen Endzustand wechseln darf.
pub const PENDING: TaskState = TaskState::Pending;

/// Gibt zurück, ob der Übergang von `from` nach `to` erlaubt ist.
pub fn can_transition(from: TaskState, to: TaskState) -> bool {
    STATE_TRANSITIONS
        .iter()
        .find(|(state, _)| *state == from)
        .map(|(_, allowed)| allowed.contains(&to))
        .unwrap_or(false)
}

/// Alle erlaubten Folgezustände von `from`.
pub fn allowed_transitions(from: TaskState) -> Vec<TaskState> {
    STATE_TRANSITIONS
        .iter()
        .find(|(state, _)| *state == from)
        .map(|(_, allowed)| allowed.to_vec())
        .unwrap_or_default()
}

/// Agent, der einen Zustand fachlich bearbeitet.
pub fn agent_for_state(state: TaskState) -> Option<&'static str> {
    match state {
        TaskState::Taizi => Some("taizi"),
        TaskState::Zhongshu => Some("zhongshu"),
        TaskState::Menxia => Some("menxia"),
        TaskState::Assigned | TaskState::Review => Some("shangshu"),
        _ => None,
    }
}

/// Agent, der eine Organisation fachlich bearbeitet.
pub fn agent_for_org(org: &str) -> Option<&'static str> {
    match org {
        "户部" => Some("hubu"),
        "礼部" => Some("libu"),
        "兵部" => Some("bingbu"),
        "刑部" => Some("xingbu"),
        "工部" => Some("gongbu"),
        "吏部" => Some("libu_hr"),
        _ => None,
    }
}

/// Zustände, die als aktive Arbeit gelten.
pub fn is_active_state(state: TaskState) -> bool {
    matches!(
        state,
        TaskState::Assigned | TaskState::Next | TaskState::Doing | TaskState::Review
    )
}

/// Prüft, ob ein Zustand ein Endzustand ist.
pub fn is_terminal(state: TaskState) -> bool {
    TERMINAL_STATES.contains(&state)
}

/// Sammlung aller Zustände für vollständige Tests.
pub fn all_states() -> HashSet<TaskState> {
    [
        TaskState::Taizi,
        TaskState::Zhongshu,
        TaskState::Menxia,
        TaskState::Assigned,
        TaskState::Next,
        TaskState::Doing,
        TaskState::Review,
        TaskState::Done,
        TaskState::Blocked,
        TaskState::Cancelled,
        TaskState::Pending,
    ]
    .into_iter()
    .collect()
}
