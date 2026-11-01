//! Konsumierung der Ausführungsaufträge.
//!
//! Der Worker liest die Aufträge aus dem Ereignisstrom `task.dispatch`,
//! beansprucht sie in der Datenbank und führt sie über das Gateway aus. Ein
//! Auftrag wird erst bestätigt, wenn die Ausführung abgeschlossen ist.
//! Dadurch wird ein Auftrag nach einem Absturz erneut zugestellt.

use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::Value;
use tokio::sync::Semaphore;
use tokio::task::JoinHandle;

use edict_core::config::Settings;
use edict_core::error::AppResult;
use edict_core::model::{topics, Task, TaskState};
use edict_core::repository::{ClaimOutcome, DispatchClaim, DispatchRepository, TaskRepository};
use edict_core::service;
use edict_db::redis_bus::StreamMessage;
use edict_db::{DispatchStore, RedisBus, TaskStore};

use crate::gateway::{self, GatewayError, GatewayRequest};

/// Konsumentengruppe des Ausführungsstroms.
const GROUP: &str = "dispatcher";

/// Abstand zwischen zwei Prüfungen auf überfällige Nachrichten.
const RECLAIM_INTERVAL: Duration = Duration::from_secs(30);

/// Zahl der Nachrichten je Lesevorgang.
const BATCH_SIZE: usize = 3;

/// Wartezeit des Lesevorgangs in Millisekunden.
const BLOCK_MS: u64 = 2000;

/// Ergebnis eines Ausführungsversuchs.
enum AttemptResult {
    /// Der Aufruf war erfolgreich.
    Succeeded,
    /// Der Aufruf ist fehlgeschlagen.
    Failed {
        /// Grund des Fehlschlags.
        reason: String,
        /// Ein Fehler, der nicht erneut versucht wird.
        permanent: bool,
    },
}

/// Worker, der Ausführungsaufträge abarbeitet.
#[derive(Clone)]
pub struct Dispatcher {
    bus: Arc<RedisBus>,
    dispatch_store: DispatchStore,
    task_store: TaskStore,
    settings: Settings,
    consumer: String,
}

impl Dispatcher {
    /// Legt den Worker auf seine Bestandteile.
    pub fn new(
        bus: Arc<RedisBus>,
        dispatch_store: DispatchStore,
        task_store: TaskStore,
        settings: Settings,
    ) -> Self {
        let consumer = format!("disp-{}", std::process::id());
        Self {
            bus,
            dispatch_store,
            task_store,
            settings,
            consumer,
        }
    }

    /// Liest fortlaufend Aufträge und führt sie aus.
    pub async fn run(&self) -> AppResult<()> {
        self.bus.ensure_group(topics::TASK_DISPATCH, GROUP).await?;
        tracing::info!("Ausführungsworker gestartet");

        let semaphore = Arc::new(Semaphore::new(self.settings.max_concurrent_dispatch.max(1)));
        let mut handles: Vec<JoinHandle<()>> = Vec::new();
        let mut last_reclaim = Instant::now();

        loop {
            handles.retain(|handle| !handle.is_finished());

            match self
                .bus
                .read_group(
                    topics::TASK_DISPATCH,
                    GROUP,
                    &self.consumer,
                    BATCH_SIZE,
                    BLOCK_MS,
                )
                .await
            {
                Ok(messages) => {
                    for message in messages {
                        self.spawn(&semaphore, &mut handles, message).await;
                    }
                }
                Err(error) => {
                    tracing::warn!(%error, "Ausführungsstrom nicht lesbar");
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            }

            if last_reclaim.elapsed() >= RECLAIM_INTERVAL {
                last_reclaim = Instant::now();
                match self
                    .bus
                    .claim_stale(
                        topics::TASK_DISPATCH,
                        GROUP,
                        &self.consumer,
                        self.reclaim_idle_ms(),
                        20,
                    )
                    .await
                {
                    Ok(messages) => {
                        tracing::info!(count = messages.len(), "überfällige Aufträge übernommen");
                        for message in messages {
                            self.spawn(&semaphore, &mut handles, message).await;
                        }
                    }
                    Err(error) => {
                        tracing::warn!(%error, "überfällige Aufträge nicht übernommen");
                    }
                }
            }
        }
    }

    /// Startet die Bearbeitung einer Nachricht im eigenen Task.
    async fn spawn(
        &self,
        semaphore: &Arc<Semaphore>,
        handles: &mut Vec<JoinHandle<()>>,
        message: StreamMessage,
    ) {
        let Ok(permit) = semaphore.clone().acquire_owned().await else {
            return;
        };
        let dispatcher = self.clone();
        handles.push(tokio::spawn(async move {
            let _permit = permit;
            if let Err(error) = dispatcher.handle(message).await {
                tracing::error!(%error, "Ausführungsauftrag fehlgeschlagen");
            }
        }));
    }

    /// Bearbeitet einen Ausführungsauftrag.
    async fn handle(&self, message: StreamMessage) -> AppResult<()> {
        let payload = &message.event.payload;
        let task_id = string_field(payload, "task_id");
        let role = string_field(payload, "agent");
        let text = string_field(payload, "message");

        if task_id.is_empty() || role.is_empty() {
            tracing::warn!(entry = %message.id, "Auftrag ohne Aufgabe oder Rolle verworfen");
            self.ack(&message).await?;
            return Ok(());
        }

        let Some(task) = self.task_store.get_task(&task_id).await? else {
            tracing::warn!(%task_id, "Aufgabe nicht gefunden, Auftrag verworfen");
            self.ack(&message).await?;
            return Ok(());
        };

        let request_id = message.event.event_id.clone();
        let max_attempts = self.settings.max_dispatch_retry.max(1);

        loop {
            let outcome = self
                .dispatch_store
                .claim_dispatch(
                    &task.task_id,
                    &request_id,
                    &self.consumer,
                    self.lease_sec(),
                )
                .await?;

            let claim = match outcome {
                ClaimOutcome::Finished => {
                    tracing::info!(%task_id, "Auftrag war bereits erfolgreich");
                    self.ack(&message).await?;
                    return Ok(());
                }
                ClaimOutcome::Busy => {
                    tracing::info!(%task_id, "Auftrag wird gerade ausgeführt");
                    return Ok(());
                }
                ClaimOutcome::Claimed(claim) => claim,
            };

            tracing::info!(
                %task_id,
                role = %role,
                attempt = claim.attempt,
                "Auftrag wird ausgeführt"
            );

            match self.run_attempt(&task, &role, &text, &claim).await {
                AttemptResult::Succeeded => {
                    self.dispatch_store
                        .finish_dispatch(&claim, true, None)
                        .await?;
                    self.ack(&message).await?;
                    tracing::info!(%task_id, "Auftrag erfolgreich ausgeführt");
                    return Ok(());
                }
                AttemptResult::Failed { reason, permanent } => {
                    self.dispatch_store
                        .finish_dispatch(&claim, false, Some(&reason))
                        .await?;

                    if permanent {
                        tracing::error!(%task_id, reason = %reason, "Auftrag endgültig gescheitert");
                        self.block_task(&task, &reason).await;
                        self.ack(&message).await?;
                        return Ok(());
                    }

                    if claim.attempt >= max_attempts {
                        tracing::error!(
                            %task_id,
                            attempt = claim.attempt,
                            reason = %reason,
                            "Auftrag nach den erlaubten Versuchen gescheitert"
                        );
                        self.block_task(&task, &reason).await;
                        self.ack(&message).await?;
                        return Ok(());
                    }

                    tracing::warn!(
                        %task_id,
                        attempt = claim.attempt,
                        reason = %reason,
                        "erneuter Versuch folgt"
                    );
                    tokio::time::sleep(backoff(claim.attempt)).await;
                }
            }
        }
    }

    /// Führt einen einzelnen Versuch aus und schreibt die Ereignisse.
    async fn run_attempt(
        &self,
        task: &Task,
        role: &str,
        text: &str,
        claim: &DispatchClaim,
    ) -> AttemptResult {
        let start_event = service::agent_dispatch_start_event(task, role);
        if let Err(error) = self.dispatch_store.append_event_with_outbox(&start_event).await {
            return AttemptResult::Failed {
                reason: format!("Beginn nicht gespeichert: {error}"),
                permanent: false,
            };
        }

        let request = GatewayRequest {
            role: role.to_string(),
            message: text.to_string(),
            task_id: task.task_id.clone(),
            legacy_id: task.id.clone(),
            trace_id: task.trace_id.clone(),
            dispatch_id: claim.dispatch_id.clone(),
        };

        let outcome = gateway::run(&self.settings, request).await;
        let (return_code, output, reason, permanent) = match outcome {
            Ok(result) => {
                let reason = if result.return_code == 0 {
                    String::new()
                } else {
                    let detail = if result.stderr.trim().is_empty() {
                        format!("Rückgabewert {}", result.return_code)
                    } else {
                        result.stderr.trim().to_string()
                    };
                    format!("Gateway {role} meldet Fehler: {detail}")
                };
                (
                    result.return_code,
                    result.stdout,
                    reason,
                    false,
                )
            }
            Err(GatewayError::TemplateMissing(template)) => (
                -1,
                String::new(),
                format!("Vorlage {template} fehlt. Der Auftrag wird nicht erneut versucht."),
                true,
            ),
            Err(GatewayError::BinaryMissing(path)) => (
                -1,
                String::new(),
                format!("Gateway {path} wurde nicht gefunden."),
                false,
            ),
            Err(GatewayError::Timeout) => (
                -1,
                String::new(),
                format!(
                    "Gateway {role} hat die Zeitgrenze von {} Sekunden überschritten.",
                    self.settings.dispatch_timeout_sec
                ),
                false,
            ),
            Err(GatewayError::StartFailed(detail)) => (
                -1,
                String::new(),
                format!("Gateway {role} ließ sich nicht starten: {detail}"),
                false,
            ),
        };

        let output_event = service::agent_output_event(task, role, &output, return_code);
        if let Err(error) = self.dispatch_store.append_event_with_outbox(&output_event).await {
            tracing::error!(%error, "Ausgabe des Agenten nicht gespeichert");
            return AttemptResult::Failed {
                reason: format!("Ausgabe nicht gespeichert: {error}"),
                permanent: false,
            };
        }

        if return_code == 0 {
            AttemptResult::Succeeded
        } else {
            AttemptResult::Failed { reason, permanent }
        }
    }

    /// Setzt die Aufgabe auf den Zustand `Blocked`.
    async fn block_task(&self, task: &Task, reason: &str) {
        match self
            .task_store
            .transition_state(&task.task_id, TaskState::Blocked, "dispatcher", reason)
            .await
        {
            Ok(_) => tracing::warn!(task = %task.task_id, reason = %reason, "Aufgabe gesperrt"),
            Err(error) => tracing::error!(
                task = %task.task_id,
                reason = %reason,
                %error,
                "Aufgabe konnte nicht gesperrt werden"
            ),
        }
    }

    /// Bestätigt eine Nachricht im Strom.
    async fn ack(&self, message: &StreamMessage) -> AppResult<()> {
        self.bus.ack(topics::TASK_DISPATCH, GROUP, &message.id).await
    }

    /// Laufzeit der Ausführungssperre in Sekunden.
    ///
    /// Die Sperre gilt länger als der Aufruf des Gateways, damit ein
    /// abgestürzter Worker seine Sperre nicht selbst freigibt.
    fn lease_sec(&self) -> i64 {
        self.settings.dispatch_timeout_sec as i64 + 60
    }

    /// Ruhezeit, ab der eine Nachricht erneut übernommen wird.
    fn reclaim_idle_ms(&self) -> u64 {
        (self.settings.dispatch_timeout_sec + 60) * 1000
    }
}

/// Liest ein Textfeld aus einer Nutzlast.
fn string_field(payload: &serde_json::Map<String, Value>, name: &str) -> String {
    payload
        .get(name)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

/// Wartezeit vor dem nächsten Versuch.
///
/// Die Wartezeit wächst mit dem Versuch und ist auf 30 Sekunden begrenzt.
fn backoff(attempt: i64) -> Duration {
    let seconds = 2u64.saturating_pow(attempt.clamp(1, 5) as u32).min(30);
    Duration::from_secs(seconds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wartezeit_wächst_bis_zum_grenzwert() {
        assert_eq!(backoff(1), Duration::from_secs(2));
        assert_eq!(backoff(3), Duration::from_secs(8));
        assert_eq!(backoff(9), Duration::from_secs(30));
    }
}
