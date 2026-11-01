//! Gestaltung der Oberflaeche.
//!
//! Die Farben folgen der Einstellung des Browsers. `prefers-color-scheme`
//! waehlt zwischen heller und dunkler Darstellung. Layout und Abstaende
//! passen sich an schmale Bildschirme an.

/// CSS der Oberflaeche.
pub const STYLES: &str = r#"
:root {
    color-scheme: light dark;
    --bg: #f4f5f7;
    --surface: #ffffff;
    --surface-alt: #eceef2;
    --border: #d3d7de;
    --text: #1b1f27;
    --text-muted: #5c6472;
    --accent: #2f5fd0;
    --accent-text: #ffffff;
    --danger: #b3261e;
    --success: #1f7a4d;
    --warning: #9a6700;
    --shadow: 0 1px 2px rgba(16, 24, 40, 0.08);
    --radius: 10px;
}

@media (prefers-color-scheme: dark) {
    :root {
        --bg: #14171d;
        --surface: #1d212a;
        --surface-alt: #262b36;
        --border: #39404d;
        --text: #e8eaee;
        --text-muted: #a3abb9;
        --accent: #6f9bff;
        --accent-text: #10131a;
        --danger: #ff8a80;
        --success: #6fd39b;
        --warning: #e8b661;
        --shadow: 0 1px 2px rgba(0, 0, 0, 0.4);
    }
}

* {
    box-sizing: border-box;
}

body {
    margin: 0;
    background: var(--bg);
    color: var(--text);
    font-family: "Segoe UI", "Noto Sans", system-ui, sans-serif;
    font-size: 15px;
    line-height: 1.5;
}

button,
input,
select,
textarea {
    font: inherit;
    color: inherit;
}

:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
}

.app-shell {
    min-height: 100vh;
    display: flex;
    flex-direction: column;
}

.app-header {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 16px;
    padding: 12px 20px;
    background: var(--surface);
    border-bottom: 1px solid var(--border);
    position: sticky;
    top: 0;
    z-index: 5;
}

.brand {
    display: flex;
    flex-direction: column;
    line-height: 1.2;
}

.brand h1 {
    margin: 0;
    font-size: 19px;
}

.brand-subtitle {
    color: var(--text-muted);
    font-size: 12px;
}

.main-nav {
    display: flex;
    gap: 8px;
}

.nav-button {
    padding: 7px 14px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface-alt);
    cursor: pointer;
}

.nav-button:hover {
    border-color: var(--accent);
}

.nav-button-active {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--accent-text);
}

.header-actions {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 12px;
}

.connection-badge {
    padding: 5px 10px;
    border-radius: 999px;
    background: var(--surface-alt);
    border: 1px solid var(--border);
    color: var(--text-muted);
    font-size: 13px;
    white-space: nowrap;
}

.primary-button,
.secondary-button {
    padding: 7px 14px;
    border-radius: var(--radius);
    border: 1px solid var(--accent);
    cursor: pointer;
}

.primary-button {
    background: var(--accent);
    color: var(--accent-text);
}

.secondary-button {
    background: var(--surface-alt);
    border-color: var(--border);
    color: var(--text);
}

.secondary-button:hover {
    border-color: var(--accent);
}

.danger-button {
    border-color: var(--danger);
    color: var(--danger);
    background: var(--surface-alt);
}

.app-main {
    flex: 1;
    padding: 20px;
}

.filter-bar {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    gap: 12px;
    padding: 14px;
    margin-bottom: 16px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    box-shadow: var(--shadow);
}

.field {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 13px;
    color: var(--text-muted);
}

.field input,
.field select,
.field textarea {
    padding: 7px 9px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--surface);
    color: var(--text);
    min-width: 160px;
}

.field textarea {
    min-height: 90px;
    resize: vertical;
}

.field-grow {
    flex: 1;
    min-width: 220px;
}

.board {
    display: flex;
    gap: 14px;
    align-items: flex-start;
    overflow-x: auto;
    padding-bottom: 12px;
}

.board-column {
    flex: 0 0 260px;
    background: var(--surface-alt);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 10px;
    max-height: calc(100vh - 190px);
    overflow-y: auto;
}

.board-column-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 10px;
    font-weight: 600;
}

.board-column-count {
    color: var(--text-muted);
    font-size: 12px;
}

.task-card {
    display: block;
    width: 100%;
    text-align: left;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 10px;
    margin-bottom: 10px;
    cursor: pointer;
    box-shadow: var(--shadow);
}

.task-card:hover {
    border-color: var(--accent);
}

.task-card-title {
    font-weight: 600;
    margin-bottom: 4px;
}

.task-card-meta {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: center;
    margin-bottom: 6px;
}

.task-card-line {
    color: var(--text-muted);
    font-size: 13px;
}

.task-card-id {
    font-family: ui-monospace, "SFMono-Regular", monospace;
    font-size: 12px;
    color: var(--text-muted);
}

.badge {
    display: inline-block;
    padding: 2px 8px;
    border-radius: 999px;
    border: 1px solid var(--border);
    font-size: 12px;
    background: var(--surface-alt);
    white-space: nowrap;
}

.badge-accent {
    border-color: var(--accent);
    color: var(--accent);
}

.badge-success {
    border-color: var(--success);
    color: var(--success);
}

.badge-warning {
    border-color: var(--warning);
    color: var(--warning);
}

.badge-danger {
    border-color: var(--danger);
    color: var(--danger);
}

.pipeline {
    display: flex;
    gap: 3px;
    margin-top: 8px;
}

.pipeline-step {
    flex: 1;
    height: 4px;
    border-radius: 2px;
    background: var(--border);
}

.pipeline-step-done {
    background: var(--accent);
}

.pipeline-step-current {
    background: var(--warning);
}

.progress {
    width: 100%;
    height: 6px;
    border-radius: 3px;
    background: var(--border);
    overflow: hidden;
    margin-top: 6px;
}

.progress-bar {
    height: 100%;
    background: var(--success);
}

.state-message {
    padding: 28px;
    text-align: center;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    color: var(--text-muted);
}

.state-message-error {
    border-color: var(--danger);
    color: var(--danger);
}

.detail-layout {
    display: grid;
    grid-template-columns: minmax(0, 2fr) minmax(280px, 1fr);
    gap: 18px;
    align-items: start;
}

.panel {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 16px;
    margin-bottom: 16px;
    box-shadow: var(--shadow);
}

.panel h2,
.panel h3 {
    margin: 0 0 10px;
    font-size: 16px;
}

.detail-title {
    margin: 0 0 8px;
    font-size: 22px;
}

.detail-meta {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-bottom: 14px;
}

.definition-list {
    display: grid;
    grid-template-columns: minmax(120px, 180px) minmax(0, 1fr);
    gap: 6px 14px;
    margin: 0;
}

.definition-list dt {
    color: var(--text-muted);
    font-size: 13px;
}

.definition-list dd {
    margin: 0;
    overflow-wrap: anywhere;
}

.action-row {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    align-items: center;
    margin-top: 10px;
}

.action-row input[type="text"] {
    flex: 1;
    min-width: 180px;
    padding: 7px 9px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--surface);
}

.todo-list {
    list-style: none;
    margin: 0;
    padding: 0;
}

.todo-item {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    padding: 6px 0;
    border-bottom: 1px solid var(--border);
}

.todo-item:last-child {
    border-bottom: none;
}

.todo-title {
    flex: 1;
    min-width: 140px;
}

.todo-status {
    padding: 4px 8px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--surface);
}

.log-list {
    list-style: none;
    margin: 0;
    padding: 0;
    max-height: 320px;
    overflow-y: auto;
}

.log-item {
    padding: 8px 0;
    border-bottom: 1px solid var(--border);
    font-size: 14px;
}

.log-item:last-child {
    border-bottom: none;
}

.log-time {
    color: var(--text-muted);
    font-size: 12px;
}

.overview-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(320px, 1fr));
    gap: 18px;
    align-items: start;
}

.table {
    width: 100%;
    border-collapse: collapse;
    font-size: 14px;
}

.table th,
.table td {
    text-align: left;
    padding: 6px 8px;
    border-bottom: 1px solid var(--border);
    vertical-align: top;
}

.table th {
    color: var(--text-muted);
    font-weight: 600;
}

.table-wrapper {
    overflow-x: auto;
}

.stat-row {
    display: flex;
    flex-wrap: wrap;
    gap: 18px;
    margin-bottom: 12px;
}

.stat {
    background: var(--surface-alt);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 10px 14px;
    min-width: 130px;
}

.stat-value {
    font-size: 20px;
    font-weight: 600;
}

.stat-label {
    color: var(--text-muted);
    font-size: 12px;
}

.news-item {
    padding: 8px 0;
    border-bottom: 1px solid var(--border);
}

.news-item:last-child {
    border-bottom: none;
}

.news-title {
    font-weight: 600;
}

.news-meta {
    color: var(--text-muted);
    font-size: 12px;
}

.result-list {
    display: grid;
    grid-template-columns: minmax(120px, 200px) minmax(0, 1fr);
    gap: 4px 12px;
    margin: 0;
    font-size: 14px;
}

.result-list dt {
    color: var(--text-muted);
}

.result-list dd {
    margin: 0;
    overflow-wrap: anywhere;
}

.modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(10, 12, 16, 0.55);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 20px;
    z-index: 20;
}

.modal {
    width: min(520px, 100%);
    max-height: 90vh;
    overflow-y: auto;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 18px;
    display: flex;
    flex-direction: column;
    gap: 12px;
}

.modal h2 {
    margin: 0;
    font-size: 18px;
}

.modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
}

.toast-stack {
    position: fixed;
    right: 16px;
    bottom: 16px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    z-index: 30;
    max-width: min(420px, calc(100vw - 32px));
}

.toast {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 12px;
    border-radius: var(--radius);
    border: 1px solid var(--border);
    background: var(--surface);
    box-shadow: var(--shadow);
}

.toast-success {
    border-color: var(--success);
}

.toast-error {
    border-color: var(--danger);
}

.toast-close {
    margin-left: auto;
    border: none;
    background: transparent;
    cursor: pointer;
    font-size: 18px;
    line-height: 1;
}

.back-link {
    border: none;
    background: transparent;
    color: var(--accent);
    cursor: pointer;
    padding: 0;
    margin-bottom: 10px;
}

@media (max-width: 900px) {
    .detail-layout {
        grid-template-columns: minmax(0, 1fr);
    }

    .app-header {
        align-items: flex-start;
    }

    .header-actions {
        margin-left: 0;
        width: 100%;
        justify-content: space-between;
    }
}

@media (max-width: 600px) {
    .app-main {
        padding: 12px;
    }

    .definition-list,
    .result-list {
        grid-template-columns: minmax(0, 1fr);
    }

    .board-column {
        flex-basis: 84vw;
    }
}

@media (prefers-reduced-motion: reduce) {
    * {
        transition: none !important;
        animation: none !important;
    }
}
"#;
