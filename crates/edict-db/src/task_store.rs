//! PostgreSQL-Persistenz für Aufgaben, Teilaufgaben und Fortschrittsmeldungen.
//!
//! Der Datenbestand bleibt die kanonische Form. Die Übersetzung ins Deutsche
//! übernimmt der Kern beim Anzeigen, siehe `edict_core::labels`.

use chrono::{DateTime, Utc};
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};
use sqlx::postgres::{PgPool, PgRow};
use sqlx::{Postgres, QueryBuilder, Row};
use uuid::Uuid;

use edict_core::error::{AppError, AppResult};
use edict_core::model::{Event, FlowEntry, LiveStatus, ProgressEntry, Task, TaskState, TodoItem};
use edict_core::repository::{EventRepository, StatisticsRepository, TaskFilter, TaskRepository};
use edict_core::service;

use crate::events::{append_event_with_outbox, parse_timestamp, parse_uuid};

/// Spaltenliste der Aufgaben. Sie wird von allen Auslesewegen benutzt.
const TASK_COLUMNS: &str = "task_id, legacy_id, trace_id, title, description, state, org, \
    official, assignee_org, creator, now_text, eta, block, output, ac, priority, archived, \
    flow_log, tags, meta, template_id, template_params, target_dept, scheduler, review_round, \
    source_meta, prev_state, activity, version, created_at, updated_at, archived_at";

/// Persistenz für Aufgaben auf Basis von PostgreSQL.
#[derive(Clone)]
pub struct TaskStore {
    pool: PgPool,
}

impl TaskStore {
    /// Legt einen Datenbestand auf dem Pool an.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Der zugrunde liegende Pool.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Liest eine Aufgabe über ihre technische oder fachliche Kennung.
    async fn load_task(&self, id: &str) -> AppResult<Option<Task>> {
        let mut connection = self
            .pool
            .acquire()
            .await
            .map_err(|error| AppError::Database(format!("Verbindung fehlt: {error}")))?;
        load_task_with(&mut connection, id, false).await
    }

    /// Liest eine Aufgabe und sperrt sie für die Dauer der Transaktion.
    async fn load_task_locked(
        &self,
        connection: &mut sqlx::PgConnection,
        id: &str,
    ) -> AppResult<Option<Task>> {
        load_task_with(connection, id, true).await
    }

    /// Speichert die Aufgabenzeile mit optimistischer Sperre.
    async fn update_task(
        &self,
        connection: &mut sqlx::PgConnection,
        task: &mut Task,
    ) -> AppResult<()> {
        let task_id = parse_uuid(&task.task_id)?;
        let expected_version = task.version;
        let updated = sqlx::query(
            "UPDATE tasks SET title = $2, description = $3, state = $4, org = $5, \
             official = $6, assignee_org = $7, creator = $8, now_text = $9, eta = $10, \
             block = $11, output = $12, ac = $13, priority = $14, archived = $15, \
             flow_log = $16, tags = $17, meta = $18, template_id = $19, \
             template_params = $20, target_dept = $21, scheduler = $22, \
             review_round = $23, source_meta = $24, prev_state = $25, activity = $26, \
             version = version + 1, updated_at = $27 WHERE task_id = $1 AND version = $28",
        )
        .bind(task_id)
        .bind(&task.title)
        .bind(&task.description)
        .bind(task.state.as_str())
        .bind(&task.org)
        .bind(&task.official)
        .bind(&task.assignee_org)
        .bind(&task.creator)
        .bind(&task.now)
        .bind(&task.eta)
        .bind(&task.block)
        .bind(&task.output)
        .bind(&task.ac)
        .bind(&task.priority)
        .bind(task.archived)
        .bind(serde_json::to_value(&task.flow_log)?)
        .bind(serde_json::to_value(&task.tags)?)
        .bind(Value::Object(task.meta.clone()))
        .bind(&task.template_id)
        .bind(Value::Object(task.template_params.clone()))
        .bind(&task.target_dept)
        .bind(Value::Object(task.scheduler.clone()))
        .bind(task.review_round)
        .bind(
            task.source_meta
                .as_ref()
                .map(|meta| Value::Object(meta.clone())),
        )
        .bind(&task.prev_state)
        .bind(Value::Array(task.activity.clone()))
        .bind(parse_timestamp(&task.updated_at)?)
        .bind(expected_version)
        .execute(&mut *connection)
        .await
        .map_err(|error| AppError::Database(format!("Aufgabe nicht gespeichert: {error}")))?;

        if updated.rows_affected() == 0 {
            return Err(AppError::Conflict(format!(
                "Aufgabe {task_id} wurde zwischenzeitlich geändert"
            )));
        }
        task.version = expected_version + 1;
        Ok(())
    }

    /// Ersetzt die Teilaufgaben einer Aufgabe.
    async fn replace_todos(
        &self,
        connection: &mut sqlx::PgConnection,
        task_id: Uuid,
        todos: &[TodoItem],
    ) -> AppResult<()> {
        sqlx::query("DELETE FROM todos WHERE task_id = $1")
            .bind(task_id)
            .execute(&mut *connection)
            .await
            .map_err(|error| AppError::Database(format!("Teilaufgaben nicht löschbar: {error}")))?;

        for (index, todo) in todos.iter().enumerate() {
            sqlx::query(
                "INSERT INTO todos (task_id, position, legacy_id, title, status, detail) \
                 VALUES ($1, $2, $3, $4, $5, $6)",
            )
            .bind(task_id)
            .bind(index as i32)
            .bind(&todo.id)
            .bind(&todo.title)
            .bind(&todo.status)
            .bind(&todo.detail)
            .execute(&mut *connection)
            .await
            .map_err(|error| {
                AppError::Database(format!("Teilaufgabe nicht gespeichert: {error}"))
            })?;
        }
        Ok(())
    }

    /// Schreibt ein Ereignis und den zugehörigen Outbox-Eintrag.
    async fn append_event_in(
        &self,
        connection: &mut sqlx::PgConnection,
        event: &Event,
    ) -> AppResult<()> {
        append_event_with_outbox(connection, event).await
    }
}

/// Liest eine Aufgabe über ihre technische oder fachliche Kennung.
///
/// Technische Kennung ist die UUID, fachliche Kennung der Wert aus dem
/// Altbestand wie `JJC-20260301-001`.
async fn load_task_with(
    connection: &mut sqlx::PgConnection,
    id: &str,
    lock: bool,
) -> AppResult<Option<Task>> {
    let lock_clause = if lock { " FOR UPDATE" } else { "" };
    let sql = format!("SELECT {TASK_COLUMNS} FROM tasks WHERE task_id = $1{lock_clause}");

    let row = match Uuid::parse_str(id.trim()) {
        Ok(task_id) => sqlx::query(&sql)
            .bind(task_id)
            .fetch_optional(&mut *connection)
            .await
            .map_err(|error| AppError::Database(format!("Aufgabe nicht lesbar: {error}")))?,
        Err(_) => None,
    };

    let row = match row {
        Some(row) => Some(row),
        None => {
            let sql = format!("SELECT {TASK_COLUMNS} FROM tasks WHERE legacy_id = $1{lock_clause}");
            sqlx::query(&sql)
                .bind(id)
                .fetch_optional(&mut *connection)
                .await
                .map_err(|error| AppError::Database(format!("Aufgabe nicht lesbar: {error}")))?
        }
    };

    let Some(row) = row else {
        return Ok(None);
    };
    let mut task = map_task(&row)?;
    let task_id = parse_uuid(&task.task_id)?;
    task.todos = load_todos(&mut *connection, task_id).await?;
    task.progress_log = load_progress(&mut *connection, task_id).await?;
    Ok(Some(task))
}

/// Liest die Teilaufgaben einer Aufgabe.
async fn load_todos(
    connection: &mut sqlx::PgConnection,
    task_id: Uuid,
) -> AppResult<Vec<TodoItem>> {
    let rows = sqlx::query(
        "SELECT legacy_id, title, status, detail FROM todos WHERE task_id = $1 ORDER BY position",
    )
    .bind(task_id)
    .fetch_all(&mut *connection)
    .await
    .map_err(|error| AppError::Database(format!("Teilaufgaben nicht lesbar: {error}")))?;

    let mut todos = Vec::with_capacity(rows.len());
    for (index, row) in rows.iter().enumerate() {
        let legacy_id: String = row.try_get("legacy_id").unwrap_or_default();
        let title: String = row.try_get("title").unwrap_or_default();
        let status: String = row.try_get("status").unwrap_or_default();
        let detail: Option<String> = row.try_get("detail").unwrap_or_default();
        todos.push(TodoItem {
            id: if legacy_id.is_empty() {
                (index + 1).to_string()
            } else {
                legacy_id
            },
            title,
            status,
            detail,
        });
    }
    Ok(todos)
}

/// Liest die Fortschrittsmeldungen einer Aufgabe.
async fn load_progress(
    connection: &mut sqlx::PgConnection,
    task_id: Uuid,
) -> AppResult<Vec<ProgressEntry>> {
    let rows = sqlx::query("SELECT payload FROM thoughts WHERE task_id = $1 ORDER BY seq")
        .bind(task_id)
        .fetch_all(&mut *connection)
        .await
        .map_err(|error| AppError::Database(format!("Fortschritt nicht lesbar: {error}")))?;

    let mut entries = Vec::with_capacity(rows.len());
    for row in rows {
        let payload: Value = row
            .try_get("payload")
            .map_err(|error| AppError::Database(format!("Fortschritt unlesbar: {error}")))?;
        match serde_json::from_value::<ProgressEntry>(payload) {
            Ok(entry) => entries.push(entry),
            Err(_) => continue,
        }
    }
    Ok(entries)
}

/// Wandelt eine Zeile in eine Aufgabe.
fn map_task(row: &PgRow) -> AppResult<Task> {
    let task_id: Uuid = row
        .try_get("task_id")
        .map_err(|error| AppError::Database(format!("Kennung unlesbar: {error}")))?;
    let legacy_id: String = row
        .try_get("legacy_id")
        .map_err(|error| AppError::Database(format!("Kennung unlesbar: {error}")))?;
    let state_text: String = row
        .try_get("state")
        .map_err(|error| AppError::Database(format!("Zustand unlesbar: {error}")))?;
    let state: TaskState = state_text.parse()?;
    let flow_log = read_json::<Vec<FlowEntry>>(row, "flow_log")?;
    let tags = read_json::<Vec<String>>(row, "tags")?;
    let meta = read_json::<Map<String, Value>>(row, "meta")?;
    let template_params = read_json::<Map<String, Value>>(row, "template_params")?;
    let scheduler = read_json::<Map<String, Value>>(row, "scheduler")?;
    let activity = read_json::<Vec<Value>>(row, "activity")?;
    let source_meta = read_optional_json::<Map<String, Value>>(row, "source_meta")?;
    let created_at: DateTime<Utc> = row
        .try_get("created_at")
        .map_err(|error| AppError::Database(format!("Zeitstempel unlesbar: {error}")))?;
    let updated_at: DateTime<Utc> = row
        .try_get("updated_at")
        .map_err(|error| AppError::Database(format!("Zeitstempel unlesbar: {error}")))?;
    let archived_at: Option<DateTime<Utc>> = row
        .try_get("archived_at")
        .map_err(|error| AppError::Database(format!("Zeitstempel unlesbar: {error}")))?;

    Ok(Task {
        task_id: task_id.to_string(),
        id: legacy_id,
        trace_id: row.try_get("trace_id").unwrap_or_default(),
        title: row.try_get("title").unwrap_or_default(),
        description: row.try_get("description").unwrap_or_default(),
        state,
        org: row.try_get("org").unwrap_or_default(),
        official: row.try_get("official").unwrap_or_default(),
        assignee_org: row.try_get("assignee_org").unwrap_or_default(),
        creator: row.try_get("creator").unwrap_or_default(),
        now: row.try_get("now_text").unwrap_or_default(),
        eta: row.try_get("eta").unwrap_or_default(),
        block: row.try_get("block").unwrap_or_default(),
        output: row.try_get("output").unwrap_or_default(),
        ac: row.try_get("ac").unwrap_or_default(),
        priority: row.try_get("priority").unwrap_or_default(),
        archived: row.try_get("archived").unwrap_or(false),
        flow_log,
        progress_log: Vec::new(),
        todos: Vec::new(),
        tags,
        meta,
        template_id: row.try_get("template_id").unwrap_or_default(),
        template_params,
        target_dept: row.try_get("target_dept").unwrap_or_default(),
        scheduler,
        review_round: row.try_get("review_round").unwrap_or(0),
        created_at: created_at.to_rfc3339(),
        updated_at: updated_at.to_rfc3339(),
        archived_at: archived_at.map(|value| value.to_rfc3339()),
        source_meta,
        prev_state: row.try_get("prev_state").unwrap_or_default(),
        heartbeat: None,
        activity,
        version: row.try_get("version").unwrap_or(1),
    })
}

/// Liest ein JSON-Feld und wandelt es in den erwarteten Typ.
fn read_json<T: DeserializeOwned>(row: &PgRow, column: &str) -> AppResult<T> {
    let value: Value = row
        .try_get(column)
        .map_err(|error| AppError::Database(format!("Feld {column} unlesbar: {error}")))?;
    serde_json::from_value(value)
        .map_err(|error| AppError::Database(format!("Feld {column} fehlerhaft: {error}")))
}

/// Liest ein optionales JSON-Feld.
fn read_optional_json<T: DeserializeOwned>(row: &PgRow, column: &str) -> AppResult<Option<T>> {
    let value: Option<Value> = row
        .try_get(column)
        .map_err(|error| AppError::Database(format!("Feld {column} unlesbar: {error}")))?;
    match value {
        Some(value) => Ok(Some(serde_json::from_value(value).map_err(|error| {
            AppError::Database(format!("Feld {column} fehlerhaft: {error}"))
        })?)),
        None => Ok(None),
    }
}

/// Wandelt eine Zeichenkette in eine UUID.
fn parse_uuid(value: &str) -> AppResult<Uuid> {
    Uuid::parse_str(value.trim())
        .map_err(|error| AppError::Validation(format!("Ungültige Kennung {value}: {error}")))
}

/// Liest einen Zeitstempel im RFC-Format oder im Format des Altbestands.
fn parse_timestamp(value: &str) -> AppResult<DateTime<Utc>> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(Utc::now());
    }
    if let Ok(parsed) = DateTime::parse_from_rfc3339(trimmed) {
        return Ok(parsed.with_timezone(&Utc));
    }
    if let Ok(parsed) = NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%d %H:%M:%S") {
        return Ok(parsed.and_utc());
    }
    Err(AppError::Validation(format!(
        "Zeitstempel {value} lässt sich nicht lesen"
    )))
}

#[async_trait::async_trait]
impl TaskRepository for TaskStore {
    async fn create_task(&self, mut task: Task) -> AppResult<Task> {
        if task.task_id.trim().is_empty() {
            task.task_id = Uuid::new_v4().to_string();
        }
        if task.trace_id.trim().is_empty() {
            task.trace_id = Uuid::new_v4().to_string();
        }
        let now = Utc::now().to_rfc3339();
        if task.created_at.trim().is_empty() {
            task.created_at = now.clone();
        }
        if task.updated_at.trim().is_empty() {
            task.updated_at = now;
        }
        task.version = 1;

        let task_id = parse_uuid(&task.task_id)?;
        let mut transaction =
            self.pool.begin().await.map_err(|error| {
                AppError::Database(format!("Transaktion fehlgeschlagen: {error}"))
            })?;

        sqlx::query(
            "INSERT INTO tasks (task_id, legacy_id, trace_id, title, description, state, org, \
             official, assignee_org, creator, now_text, eta, block, output, ac, priority, \
             archived, flow_log, tags, meta, template_id, template_params, target_dept, \
             scheduler, review_round, source_meta, prev_state, activity, version, created_at, \
             updated_at, archived_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, \
             $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24, $25, $26, $27, $28, \
             $29, $30, $31, $32, $33)",
        )
        .bind(task_id)
        .bind(&task.id)
        .bind(&task.trace_id)
        .bind(&task.title)
        .bind(&task.description)
        .bind(task.state.as_str())
        .bind(&task.org)
        .bind(&task.official)
        .bind(&task.assignee_org)
        .bind(&task.creator)
        .bind(&task.now)
        .bind(&task.eta)
        .bind(&task.block)
        .bind(&task.output)
        .bind(&task.ac)
        .bind(&task.priority)
        .bind(task.archived)
        .bind(serde_json::to_value(&task.flow_log)?)
        .bind(serde_json::to_value(&task.tags)?)
        .bind(Value::Object(task.meta.clone()))
        .bind(&task.template_id)
        .bind(Value::Object(task.template_params.clone()))
        .bind(&task.target_dept)
        .bind(Value::Object(task.scheduler.clone()))
        .bind(task.review_round)
        .bind(
            task.source_meta
                .as_ref()
                .map(|meta| Value::Object(meta.clone())),
        )
        .bind(&task.prev_state)
        .bind(Value::Array(task.activity.clone()))
        .bind(task.version)
        .bind(parse_timestamp(&task.created_at)?)
        .bind(parse_timestamp(&task.updated_at)?)
        .bind(match &task.archived_at {
            Some(value) => Some(parse_timestamp(value)?),
            None => None,
        })
        .execute(&mut *transaction)
        .await
        .map_err(|error| AppError::Database(format!("Aufgabe nicht angelegt: {error}")))?;

        let connection = &mut *transaction;
        self.replace_todos(&mut *connection, task_id, &task.todos)
            .await?;
        for entry in &task.progress_log {
            sqlx::query(
                "INSERT INTO thoughts (task_id, agent, agent_label, content, payload) \
                 VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(task_id)
            .bind(entry.agent.clone().unwrap_or_default())
            .bind(entry.agent_label.clone().unwrap_or_default())
            .bind(entry.content.clone().unwrap_or_default())
            .bind(serde_json::to_value(entry)?)
            .execute(&mut *connection)
            .await
            .map_err(|error| {
                AppError::Database(format!("Fortschritt nicht gespeichert: {error}"))
            })?;
        }

        let event = service::task_created_event(&task);
        self.append_event_in(&mut *connection, &event).await?;

        transaction
            .commit()
            .await
            .map_err(|error| AppError::Database(format!("Transaktion fehlgeschlagen: {error}")))?;
        Ok(task)
    }

    async fn get_task(&self, id: &str) -> AppResult<Option<Task>> {
        self.load_task(id).await
    }

    async fn list_tasks(&self, filter: &TaskFilter) -> AppResult<Vec<Task>> {
        let mut builder = QueryBuilder::<Postgres>::new("SELECT ");
        builder.push(TASK_COLUMNS);
        builder.push(" FROM tasks WHERE 1 = 1");
        if let Some(state) = filter.state {
            builder.push(" AND state = ").push_bind(state.as_str());
        }
        if let Some(org) = &filter.org {
            builder.push(" AND org = ").push_bind(org.clone());
        }
        if let Some(assignee) = &filter.assignee_org {
            builder
                .push(" AND assignee_org = ")
                .push_bind(assignee.clone());
        }
        if let Some(priority) = &filter.priority {
            builder.push(" AND priority = ").push_bind(priority.clone());
        }
        if let Some(archived) = filter.archived {
            builder.push(" AND archived = ").push_bind(archived);
        }
        builder.push(" ORDER BY updated_at DESC, legacy_id ASC");
        if let Some(limit) = filter.limit {
            builder.push(" LIMIT ").push_bind(limit);
        }
        if let Some(offset) = filter.offset {
            builder.push(" OFFSET ").push_bind(offset);
        }

        let rows = builder
            .build()
            .fetch_all(&self.pool)
            .await
            .map_err(|error| AppError::Database(format!("Aufgaben nicht lesbar: {error}")))?;

        let mut tasks = Vec::with_capacity(rows.len());
        for row in &rows {
            let mut task = map_task(row)?;
            let task_id = parse_uuid(&task.task_id)?;
            let mut connection = self
                .pool
                .acquire()
                .await
                .map_err(|error| AppError::Database(format!("Verbindung fehlt: {error}")))?;
            task.todos = load_todos(&mut connection, task_id).await?;
            task.progress_log = load_progress(&mut connection, task_id).await?;
            tasks.push(task);
        }
        Ok(tasks)
    }

    async fn count_by_state(&self) -> AppResult<Vec<(String, i64)>> {
        let rows = sqlx::query("SELECT state, COUNT(*) AS total FROM tasks GROUP BY state")
            .fetch_all(&self.pool)
            .await
            .map_err(|error| AppError::Database(format!("Zählung fehlgeschlagen: {error}")))?;
        let mut counts = Vec::with_capacity(rows.len());
        for row in rows {
            let state: String = row.try_get("state").unwrap_or_default();
            let total: i64 = row.try_get("total").unwrap_or(0);
            counts.push((state, total));
        }
        Ok(counts)
    }

    async fn save_task(&self, task: &mut Task) -> AppResult<()> {
        let mut connection = self
            .pool
            .acquire()
            .await
            .map_err(|error| AppError::Database(format!("Verbindung fehlt: {error}")))?;
        self.update_task(&mut connection, task).await
    }

    async fn set_archived(&self, id: &str, archived: bool) -> AppResult<bool> {
        let mut transaction =
            self.pool.begin().await.map_err(|error| {
                AppError::Database(format!("Transaktion fehlgeschlagen: {error}"))
            })?;
        let connection = &mut *transaction;

        let Some(mut task) = self.load_task_locked(connection, id).await? else {
            return Err(AppError::TaskNotFound(id.to_string()));
        };
        if task.archived == archived {
            transaction.commit().await.map_err(|error| {
                AppError::Database(format!("Transaktion fehlgeschlagen: {error}"))
            })?;
            return Ok(false);
        }
        service::apply_archived(&mut task, archived);
        self.update_task(connection, &mut task).await?;
        transaction
            .commit()
            .await
            .map_err(|error| AppError::Database(format!("Transaktion fehlgeschlagen: {error}")))?;
        Ok(true)
    }

    async fn find_by_legacy_id(&self, legacy_id: &str) -> AppResult<Option<Task>> {
        let mut connection = self
            .pool
            .acquire()
            .await
            .map_err(|error| AppError::Database(format!("Verbindung fehlt: {error}")))?;
        let row = sqlx::query(&format!(
            "SELECT {TASK_COLUMNS} FROM tasks WHERE legacy_id = $1"
        ))
        .bind(legacy_id)
        .fetch_optional(&mut *connection)
        .await
        .map_err(|error| AppError::Database(format!("Aufgabe nicht lesbar: {error}")))?;
        let Some(row) = row else {
            return Ok(None);
        };
        let mut task = map_task(&row)?;
        let task_id = parse_uuid(&task.task_id)?;
        task.todos = load_todos(&mut connection, task_id).await?;
        task.progress_log = load_progress(&mut connection, task_id).await?;
        Ok(Some(task))
    }

    async fn transition_state(
        &self,
        id: &str,
        new_state: TaskState,
        agent: &str,
        reason: &str,
    ) -> AppResult<Task> {
        let mut transaction =
            self.pool.begin().await.map_err(|error| {
                AppError::Database(format!("Transaktion fehlgeschlagen: {error}"))
            })?;
        let connection = &mut *transaction;

        let Some(mut task) = self.load_task_locked(connection, id).await? else {
            return Err(AppError::TaskNotFound(id.to_string()));
        };
        let from = task.state;
        service::apply_transition(&mut task, new_state, agent, reason)?;
        self.update_task(connection, &mut task).await?;

        let event = service::state_change_event(&task, from, new_state, reason, agent);
        self.append_event_in(connection, &event).await?;

        transaction
            .commit()
            .await
            .map_err(|error| AppError::Database(format!("Transaktion fehlgeschlagen: {error}")))?;
        Ok(task)
    }

    async fn set_progress(
        &self,
        id: &str,
        agent: &str,
        content: &str,
        agent_label: Option<&str>,
    ) -> AppResult<Task> {
        let mut transaction =
            self.pool.begin().await.map_err(|error| {
                AppError::Database(format!("Transaktion fehlgeschlagen: {error}"))
            })?;
        let connection = &mut *transaction;

        let Some(mut task) = self.load_task_locked(connection, id).await? else {
            return Err(AppError::TaskNotFound(id.to_string()));
        };
        let entry = service::apply_progress(&mut task, agent, content, agent_label);
        self.update_task(connection, &mut task).await?;

        let task_id = parse_uuid(&task.task_id)?;
        sqlx::query(
            "INSERT INTO thoughts (task_id, agent, agent_label, content, payload) \
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(task_id)
        .bind(agent)
        .bind(agent_label.unwrap_or_default())
        .bind(content)
        .bind(serde_json::to_value(&entry)?)
        .execute(&mut *connection)
        .await
        .map_err(|error| AppError::Database(format!("Fortschritt nicht gespeichert: {error}")))?;

        transaction
            .commit()
            .await
            .map_err(|error| AppError::Database(format!("Transaktion fehlgeschlagen: {error}")))?;
        Ok(task)
    }

    async fn set_todos(&self, id: &str, todos: Vec<TodoItem>) -> AppResult<Task> {
        let mut transaction =
            self.pool.begin().await.map_err(|error| {
                AppError::Database(format!("Transaktion fehlgeschlagen: {error}"))
            })?;
        let connection = &mut *transaction;

        let Some(mut task) = self.load_task_locked(connection, id).await? else {
            return Err(AppError::TaskNotFound(id.to_string()));
        };
        service::apply_todos(&mut task, todos);
        self.update_task(connection, &mut task).await?;
        let task_id = parse_uuid(&task.task_id)?;
        self.replace_todos(connection, task_id, &task.todos).await?;

        transaction
            .commit()
            .await
            .map_err(|error| AppError::Database(format!("Transaktion fehlgeschlagen: {error}")))?;
        Ok(task)
    }

    async fn set_scheduler(&self, id: &str, scheduler: Map<String, Value>) -> AppResult<Task> {
        let mut transaction =
            self.pool.begin().await.map_err(|error| {
                AppError::Database(format!("Transaktion fehlgeschlagen: {error}"))
            })?;
        let connection = &mut *transaction;

        let Some(mut task) = self.load_task_locked(connection, id).await? else {
            return Err(AppError::TaskNotFound(id.to_string()));
        };
        service::apply_scheduler(&mut task, scheduler);
        self.update_task(connection, &mut task).await?;

        transaction
            .commit()
            .await
            .map_err(|error| AppError::Database(format!("Transaktion fehlgeschlagen: {error}")))?;
        Ok(task)
    }

    async fn request_dispatch(&self, id: &str, agent: &str, message: &str) -> AppResult<Task> {
        let mut transaction =
            self.pool.begin().await.map_err(|error| {
                AppError::Database(format!("Transaktion fehlgeschlagen: {error}"))
            })?;
        let connection = &mut *transaction;

        let Some(task) = self.load_task_locked(connection, id).await? else {
            return Err(AppError::TaskNotFound(id.to_string()));
        };
        let event = service::dispatch_event(&task, agent, message);
        self.append_event_in(connection, &event).await?;

        transaction
            .commit()
            .await
            .map_err(|error| AppError::Database(format!("Transaktion fehlgeschlagen: {error}")))?;
        Ok(task)
    }

    async fn live_status(&self) -> AppResult<LiveStatus> {
        let tasks = self
            .list_tasks(&TaskFilter {
                limit: Some(500),
                ..TaskFilter::default()
            })
            .await?;

        let mut sync_status = Map::new();
        sync_status.insert("ok".to_string(), Value::Bool(true));
        sync_status.insert("taskCount".to_string(), Value::Number(tasks.len().into()));
        sync_status.insert(
            "generatedAt".to_string(),
            Value::String(Utc::now().to_rfc3339()),
        );

        Ok(LiveStatus {
            tasks,
            sync_status,
            generated_at: Some(Utc::now().to_rfc3339()),
            task_source: Some("database".to_string()),
            officials: None,
            history: None,
            metrics: None,
            health: None,
        })
    }
}

#[async_trait::async_trait]
impl EventRepository for TaskStore {
    async fn append_event(&self, event: &Event) -> AppResult<()> {
        let mut connection = self
            .pool
            .acquire()
            .await
            .map_err(|error| AppError::Database(format!("Verbindung fehlt: {error}")))?;
        self.append_event_in(&mut connection, event).await
    }

    async fn list_events(
        &self,
        trace_id: Option<&str>,
        topic: Option<&str>,
        producer: Option<&str>,
        limit: i64,
    ) -> AppResult<Vec<Event>> {
        let mut builder = QueryBuilder::<Postgres>::new(
            "SELECT event_id, trace_id, topic, event_type, producer, payload, meta, timestamp \
             FROM events WHERE 1 = 1",
        );
        if let Some(value) = trace_id {
            builder
                .push(" AND trace_id = ")
                .push_bind(value.to_string());
        }
        if let Some(value) = topic {
            builder.push(" AND topic = ").push_bind(value.to_string());
        }
        if let Some(value) = producer {
            builder
                .push(" AND producer = ")
                .push_bind(value.to_string());
        }
        builder
            .push(" ORDER BY timestamp DESC LIMIT ")
            .push_bind(limit);

        let rows = builder
            .build()
            .fetch_all(&self.pool)
            .await
            .map_err(|error| AppError::Database(format!("Ereignisse nicht lesbar: {error}")))?;

        let mut events = Vec::with_capacity(rows.len());
        for row in rows {
            let event_id: Uuid = row
                .try_get("event_id")
                .map_err(|error| AppError::Database(format!("Kennung unlesbar: {error}")))?;
            let timestamp: DateTime<Utc> = row
                .try_get("timestamp")
                .map_err(|error| AppError::Database(format!("Zeitstempel unlesbar: {error}")))?;
            events.push(Event {
                event_id: event_id.to_string(),
                trace_id: row.try_get("trace_id").unwrap_or_default(),
                timestamp: timestamp.to_rfc3339(),
                topic: row.try_get("topic").unwrap_or_default(),
                event_type: row.try_get("event_type").unwrap_or_default(),
                producer: row.try_get("producer").unwrap_or_default(),
                payload: read_json::<Map<String, Value>>(&row, "payload")?,
                meta: read_json::<Map<String, Value>>(&row, "meta")?,
            });
        }
        Ok(events)
    }
}

#[async_trait::async_trait]
impl StatisticsRepository for TaskStore {
    async fn count_by_state_and_org(&self) -> AppResult<Vec<(String, String, i64)>> {
        let rows = sqlx::query(
            "SELECT state, org, COUNT(*) AS total FROM tasks GROUP BY state, org \
             ORDER BY state, org",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|error| AppError::Database(format!("Zählung fehlgeschlagen: {error}")))?;

        let mut counts = Vec::with_capacity(rows.len());
        for row in rows {
            counts.push((
                row.try_get("state").unwrap_or_default(),
                row.try_get("org").unwrap_or_default(),
                row.try_get("total").unwrap_or(0),
            ));
        }
        Ok(counts)
    }
}
