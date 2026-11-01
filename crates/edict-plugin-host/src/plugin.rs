//! Ein einzelnes Plugin im Host.
//!
//! Der Griff hält Manifest, Prozess, Zustand und Ereigniswarteschlange.
//! Die Aufgaben des Griffs sind:
//!
//! - den Kindprozess starten und den Handshake führen,
//! - Ereignisse aus der Warteschlange an das Plugin senden,
//! - Aufrufe in beide Richtungen ausführen,
//! - den Zustand nach dem Grad der Arbeitsfähigkeit führen.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use edict_core::{topics, AppError, AppResult, Event};
use edict_plugin_protocol::{
    method, InitializeResult, InvokeResult, Message, RpcError, RPC_INVALID_PARAMS,
    RPC_METHOD_NOT_FOUND, RPC_PLUGIN_ERROR, PROTOCOL_VERSION,
};
use serde_json::{json, Value};
use tokio::sync::{mpsc, RwLock};

use crate::env::build_plugin_env;
use crate::manifest::Manifest;
use crate::state::PluginStatus;
use crate::transport::{HostRequestHandler, PluginProcess};

/// Vorgaben des Hosts für Prozessstart, Aufrufe und Wiederanlauf.
#[derive(Debug, Clone)]
pub struct HostConfig {
    /// Frist für einen Aufruf an ein Plugin.
    pub dispatch_timeout: Duration,
    /// Länge der Ereigniswarteschlange je Plugin.
    pub event_queue_capacity: usize,
    /// Frist für das geordnete Beenden.
    pub shutdown_grace: Duration,
    /// Höchstzahl der Startversuche bei einem Neustart.
    pub restart_max_attempts: usize,
    /// Wartezeit vor dem ersten Startversuch.
    pub restart_backoff_base: Duration,
    /// Faktor, mit dem die Wartezeit bei jedem weiteren Versuch wächst.
    pub restart_backoff_factor: u32,
    /// Version des Hosts für den Handshake.
    pub host_version: String,
}

impl Default for HostConfig {
    fn default() -> Self {
        Self {
            dispatch_timeout: Duration::from_secs(300),
            event_queue_capacity: 128,
            shutdown_grace: Duration::from_secs(5),
            restart_max_attempts: 3,
            restart_backoff_base: Duration::from_millis(200),
            restart_backoff_factor: 2,
            host_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

impl HostConfig {
    /// Übernimmt die Frist aus der Konfiguration des Systems.
    pub fn with_dispatch_timeout(timeout_sec: u64) -> Self {
        Self {
            dispatch_timeout: Duration::from_secs(timeout_sec),
            ..Self::default()
        }
    }
}

/// Brücke vom Plugin zurück zum Host.
///
/// Das Plugin kann den Host um Fähigkeiten bitten und Domainereignisse
/// melden. Beide Wege laufen über diese Brücke.
pub struct HostBridge {
    /// Fähigkeiten des Hosts, etwa `task.lookup`.
    pub services: Arc<dyn crate::services::HostServices>,
    /// Ablage für Domainereignisse.
    pub sink: Arc<dyn crate::services::EventSink>,
}

#[async_trait::async_trait]
impl HostRequestHandler for HostBridge {
    async fn handle(&self, methode: &str, params: Value) -> Result<Value, RpcError> {
        match methode {
            method::INVOKE => {
                let capability = params
                    .get("capability")
                    .and_then(Value::as_str)
                    .ok_or_else(|| RpcError {
                        code: RPC_INVALID_PARAMS,
                        message: "Aufruf ohne Fähigkeit".to_string(),
                        data: None,
                    })?
                    .to_string();
                let aufruf_params = params.get("params").cloned().unwrap_or(json!({}));
                match self.services.invoke(&capability, &aufruf_params).await {
                    Ok(wert) => Ok(serde_json::to_value(InvokeResult { value: wert }).unwrap_or(
                        json!({"value": Value::Null}),
                    )),
                    Err(fehler) => Err(RpcError {
                        code: RPC_PLUGIN_ERROR,
                        message: fehler.to_string(),
                        data: None,
                    }),
                }
            }
            method::EMIT_EVENT => {
                let topic = params
                    .get("topic")
                    .and_then(Value::as_str)
                    .ok_or_else(|| RpcError {
                        code: RPC_INVALID_PARAMS,
                        message: "Ereignis ohne Thema".to_string(),
                        data: None,
                    })?
                    .to_string();
                let event_type = params
                    .get("event_type")
                    .and_then(Value::as_str)
                    .unwrap_or(topic.as_str())
                    .to_string();
                let producer = params
                    .get("producer")
                    .and_then(Value::as_str)
                    .unwrap_or("plugin")
                    .to_string();
                let trace_id = params
                    .get("trace_id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let payload = params
                    .get("payload")
                    .and_then(Value::as_object)
                    .cloned()
                    .unwrap_or_default();
                let meta = params
                    .get("meta")
                    .and_then(Value::as_object)
                    .cloned()
                    .unwrap_or_default();

                let ereignis = Event {
                    event_id: uuid::Uuid::new_v4().to_string(),
                    trace_id,
                    timestamp: Utc::now().to_rfc3339(),
                    topic,
                    event_type,
                    producer,
                    payload,
                    meta,
                };
                match self.sink.append_event(&ereignis).await {
                    Ok(()) => Ok(json!({"event_id": ereignis.event_id})),
                    Err(fehler) => Err(RpcError {
                        code: RPC_PLUGIN_ERROR,
                        message: fehler.to_string(),
                        data: None,
                    }),
                }
            }
            other => Err(RpcError {
                code: RPC_METHOD_NOT_FOUND,
                message: format!("Der Host kennt die Methode {other} nicht"),
                data: None,
            }),
        }
    }
}

/// Zustandsmeldung eines Plugins für die Registry.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PluginInfo {
    /// Name des Plugins.
    pub name: String,
    /// Version des Plugins.
    pub version: String,
    /// Zustand im Host.
    pub state: PluginStatus,
    /// Fähigkeiten des Plugins.
    pub capabilities: Vec<String>,
    /// Abonnierte Ereignisarten.
    pub subscriptions: Vec<String>,
    /// HTTP-Adresse des Plugins, falls es eine meldet.
    pub http_base_url: Option<String>,
    /// Anzahl der verworfenen Ereignisse.
    pub dropped_events: u64,
    /// Länge der Ereigniswarteschlange.
    pub queued_events: u64,
    /// Sekunden seit dem Start des Prozesses.
    pub uptime_seconds: u64,
    /// Text des letzten Fehlers.
    pub last_error: Option<String>,
}

/// Ein geladenes Plugin.
pub struct PluginHandle {
    manifest: Manifest,
    dir: PathBuf,
    config: Arc<HostConfig>,
    zustand: RwLock<PluginStatus>,
    info: RwLock<InitializeResult>,
    prozess: RwLock<Option<Arc<PluginProcess>>>,
    sender: mpsc::Sender<Event>,
    verworfen: AtomicU64,
    letzter_fehler: RwLock<Option<String>>,
    gestartet: RwLock<Option<std::time::Instant>>,
}

impl PluginHandle {
    /// Legt den Griff für ein Plugin an, ohne den Prozess zu starten.
    pub fn new(manifest: Manifest, dir: PathBuf, config: Arc<HostConfig>) -> Arc<Self> {
        let (sender, receiver) = mpsc::channel(config.event_queue_capacity.max(1));
        let handle = Arc::new(Self {
            manifest,
            dir,
            config,
            zustand: RwLock::new(PluginStatus::NotLoaded),
            info: RwLock::new(InitializeResult {
                protocol_version: PROTOCOL_VERSION,
                name: String::new(),
                version: String::new(),
                capabilities: Vec::new(),
                subscriptions: Vec::new(),
                http_base_url: None,
            }),
            prozess: RwLock::new(None),
            sender,
            verworfen: AtomicU64::new(0),
            letzter_fehler: RwLock::new(None),
            gestartet: RwLock::new(None),
        });
        // Die Warteschlange wird von einer eigenen Task geleert. Der Griff
        // hält nur den Sender, damit ein hängendes Plugin den Host nicht
        // am Ende festhält.
        let schwach = Arc::downgrade(&handle);
        tokio::spawn(async move {
            while let Some(ereignis) = receiver.recv().await {
                let Some(handle) = schwach.upgrade() else {
                    break;
                };
                handle.sende_ereignis(ereignis).await;
            }
        });
        handle
    }

    /// Name des Plugins.
    pub fn name(&self) -> &str {
        &self.manifest.name
    }

    /// Verzeichnis des Plugins.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Aktueller Zustand.
    pub async fn zustand(&self) -> PluginStatus {
        *self.zustand.read().await
    }

    /// Antwort des Plugins auf den Handshake.
    pub async fn info(&self) -> InitializeResult {
        self.info.read().await.clone()
    }

    /// Startet den Kindprozess und führt den Handshake aus.
    pub async fn start(&self, bridge: Arc<HostBridge>) -> AppResult<()> {
        *self.zustand.write().await = PluginStatus::Starting;
        *self.letzter_fehler.write().await = None;

        let kommando = self.manifest.resolve_command(&self.dir)?;
        let umgebung = build_plugin_env(&self.dir, &crate::env::host_environment());
        let prozess = match PluginProcess::start(&kommando, &self.dir, &umgebung, bridge).await {
            Ok(prozess) => prozess,
            Err(fehler) => {
                *self.zustand.write().await = PluginStatus::Failed;
                *self.letzter_fehler.write().await = Some(fehler.to_string());
                return Err(fehler);
            }
        };
        *self.prozess.write().await = Some(prozess.clone());

        let params = json!({
            "protocol_version": PROTOCOL_VERSION,
            "host_version": self.config.host_version,
            "plugin_dir": self.dir.to_string_lossy(),
            "config": {},
            "limits": {
                "event_queue": self.config.event_queue_capacity,
                "call_timeout_sec": self.config.dispatch_timeout.as_secs(),
            },
        });

        let antwort = match prozess
            .ruf(method::INITIALIZE, params, self.config.dispatch_timeout)
            .await
        {
            Ok(antwort) => antwort,
            Err(fehler) => {
                self.beende_prozess().await;
                *self.zustand.write().await = PluginStatus::Failed;
                *self.letzter_fehler.write().await = Some(fehler.to_string());
                return Err(fehler);
            }
        };

        let info: InitializeResult = match serde_json::from_value(antwort) {
            Ok(info) => info,
            Err(fehler) => {
                self.beende_prozess().await;
                *self.zustand.write().await = PluginStatus::Failed;
                let text = format!("Antwort auf initialize ist unlesbar: {fehler}");
                *self.letzter_fehler.write().await = Some(text.clone());
                return Err(AppError::Validation(text));
            }
        };

        if info.protocol_version != PROTOCOL_VERSION
            || self.manifest.protocol_version != PROTOCOL_VERSION
        {
            self.beende_prozess().await;
            *self.zustand.write().await = PluginStatus::Failed;
            let text = format!(
                "Protokollversion des Plugins {} passt nicht zur Version {PROTOCOL_VERSION}",
                info.protocol_version
            );
            *self.letzter_fehler.write().await = Some(text.clone());
            return Err(AppError::Validation(text));
        }

        *self.info.write().await = info;
        *self.gestartet.write().await = Some(std::time::Instant::now());
        *self.zustand.write().await = PluginStatus::Ready;
        Ok(())
    }

    /// Beendet den Kindprozess mit Frist und danach endgültig.
    pub async fn stop(&self) -> AppResult<()> {
        let prozess = self.prozess.read().await.clone();
        if let Some(prozess) = prozess {
            let _ = prozess
                .ruf(
                    method::SHUTDOWN,
                    json!({}),
                    self.config.shutdown_grace,
                )
                .await;
            if tokio::time::timeout(self.config.shutdown_grace, prozess.lebt())
                .await
                .map(|lebt| !lebt)
                .unwrap_or(false)
            {
                self.beende_prozess().await;
            } else {
                self.beende_prozess().await;
            }
        }
        *self.prozess.write().await = None;
        *self.gestartet.write().await = None;
        *self.zustand.write().await = PluginStatus::Stopped;
        Ok(())
    }

    /// Beendet den Kindprozess ohne Frist.
    async fn beende_prozess(&self) {
        if let Some(prozess) = self.prozess.read().await.clone() {
            prozess.beende().await;
        }
    }

    /// Ruft eine Fähigkeit des Plugins auf.
    pub async fn invoke(&self, capability: &str, params: Value) -> AppResult<Value> {
        let prozess = self.prozess.read().await.clone();
        let prozess = prozess.ok_or_else(|| {
            AppError::Conflict(format!("Plugin {} läuft nicht", self.manifest.name))
        })?;
        let antwort = prozess
            .ruf(
                method::INVOKE,
                json!({"capability": capability, "params": params}),
                self.config.dispatch_timeout,
            )
            .await?;
        let ergebnis: InvokeResult = serde_json::from_value(antwort)?;
        Ok(ergebnis.value)
    }

    /// Fragt den Zustand des Plugins ab.
    pub async fn health(&self) -> AppResult<edict_plugin_protocol::HealthResult> {
        let prozess = self.prozess.read().await.clone();
        let prozess = prozess.ok_or_else(|| {
            AppError::Conflict(format!("Plugin {} läuft nicht", self.manifest.name))
        })?;
        let antwort = prozess
            .ruf(method::HEALTH, json!({}), self.config.dispatch_timeout)
            .await?;
        Ok(serde_json::from_value(antwort)?)
    }

    /// Legt ein Ereignis in die Warteschlange.
    ///
    /// Ist die Warteschlange voll, zählt der Host das Ereignis als verworfen
    /// und meldet das Plugin als eingeschränkt. Der Host wartet dabei nicht.
    pub async fn queue(&self, ereignis: Event) -> bool {
        if !self.ist_abonniert(&ereignis.topic) {
            return true;
        }
        match self.sender.try_send(ereignis) {
            Ok(()) => true,
            Err(mpsc::error::TrySendError::Full(_)) => {
                self.verworfen.fetch_add(1, Ordering::SeqCst);
                let mut zustand = self.zustand.write().await;
                if *zustand == PluginStatus::Ready {
                    *zustand = PluginStatus::Degraded;
                }
                false
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                self.verworfen.fetch_add(1, Ordering::SeqCst);
                false
            }
        }
    }

    /// Prüft, ob das Plugin das Thema abonniert hat.
    pub fn ist_abonniert(&self, topic: &str) -> bool {
        let info = self.info.try_read();
        let abonnements = match info {
            Ok(info) => info.subscriptions.clone(),
            Err(_) => self.manifest.subscriptions.clone(),
        };
        if abonnements.is_empty() {
            return false;
        }
        abonnements
            .iter()
            .any(|muster| muster == "*" || muster == topic)
    }

    /// Sendet ein Ereignis an das Plugin.
    ///
    /// Das Plugin erhält nur den Nutzlastanteil des Ereignisses.
    async fn sende_ereignis(&self, ereignis: Event) {
        let prozess = self.prozess.read().await.clone();
        let Some(prozess) = prozess else {
            return;
        };
        let mut zustand = self.zustand.write().await;
        if *zustand != PluginStatus::Ready && *zustand != PluginStatus::Degraded {
            return;
        }
        let params = Value::Object(ereignis.payload.clone());
        match prozess.hinweis(method::EVENT, params).await {
            Ok(()) => {
                if *zustand == PluginStatus::Degraded && self.sender.capacity() > self.sender.len() {
                    *zustand = PluginStatus::Ready;
                }
            }
            Err(fehler) => {
                tracing::warn!(plugin = %self.manifest.name, "Ereignis konnte nicht gesendet werden: {fehler}");
                *self.letzter_fehler.write().await = Some(fehler.to_string());
            }
        }
    }

    /// Zustandsmeldung für die Registry.
    pub async fn plugin_info(&self) -> PluginInfo {
        let info = self.info.read().await.clone();
        let gestartet = self.gestartet.read().await;
        let uptime_seconds = gestartet
            .map(|zeit| zeit.elapsed().as_secs())
            .unwrap_or_default();
        PluginInfo {
            name: self.manifest.name.clone(),
            version: info.version.clone(),
            state: *self.zustand.read().await,
            capabilities: info.capabilities.clone(),
            subscriptions: info.subscriptions.clone(),
            http_base_url: info.http_base_url.clone(),
            dropped_events: self.verworfen.load(Ordering::SeqCst),
            queued_events: self.sender.len() as u64,
            uptime_seconds,
            last_error: self.letzter_fehler.read().await.clone(),
        }
    }

    /// Anzahl der verworfenen Ereignisse.
    pub fn verworfene_ereignisse(&self) -> u64 {
        self.verworfen.load(Ordering::SeqCst)
    }

    /// Meldet einen Fehler des Plugins im Zustand.
    pub async fn melde_fehler(&self, text: impl Into<String>) {
        let text = text.into();
        tracing::warn!(plugin = %self.manifest.name, "{text}");
        *self.letzter_fehler.write().await = Some(text);
        let mut zustand = self.zustand.write().await;
        if *zustand != PluginStatus::Stopped {
            *zustand = PluginStatus::Failed;
        }
    }
}

/// Baut eine Ereignisnutzlast für den Prüfkanal.
pub fn review_request_event(task_id: &str, dateien: &[String]) -> Event {
    let mut payload = serde_json::Map::new();
    payload.insert("task_id".to_string(), Value::String(task_id.to_string()));
    payload.insert(
        "files".to_string(),
        Value::Array(dateien.iter().map(|name| Value::String(name.clone())).collect()),
    );
    Event {
        event_id: uuid::Uuid::new_v4().to_string(),
        trace_id: String::new(),
        timestamp: Utc::now().to_rfc3339(),
        topic: topics::TASK_REVIEW_REQUEST.to_string(),
        event_type: "task.review.request".to_string(),
        producer: "plugin_host".to_string(),
        payload,
        meta: serde_json::Map::new(),
    }
}

/// Wandelt eine Nachricht in einen Fehler mit Code.
pub fn meldungsfehler(text: &str) -> RpcError {
    RpcError {
        code: RPC_INVALID_PARAMS,
        message: text.to_string(),
        data: None,
    }
}

/// Baut eine Antwortnachricht für Fehlerfälle.
pub fn fehlernachricht(kennung: Value, fehler: RpcError) -> Message {
    Message::error_response(kennung, fehler.code, fehler.message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> Manifest {
        Manifest {
            name: "test".to_string(),
            version: "1.0.0".to_string(),
            protocol_version: PROTOCOL_VERSION,
            command: "./test".to_string(),
            capabilities: Vec::new(),
            subscriptions: vec!["task.review.request".to_string()],
            description: None,
        }
    }

    #[tokio::test]
    async fn abonnements_steuern_die_zustellung() {
        let handle = PluginHandle::new(
            manifest(),
            PathBuf::from("/tmp"),
            Arc::new(HostConfig::default()),
        );
        assert!(handle.ist_abonniert("task.review.request"));
        assert!(!handle.ist_abonniert("task.created"));
    }

    #[tokio::test]
    async fn volle_warteschlange_verwirft_ereignisse_ohne_blockieren() {
        let config = HostConfig {
            event_queue_capacity: 2,
            ..HostConfig::default()
        };
        let handle = PluginHandle::new(manifest(), PathBuf::from("/tmp"), Arc::new(config));
        for _ in 0..8 {
            handle
                .queue(review_request_event("JJC-1", &["a.rs".to_string()]))
                .await;
        }
        assert!(handle.verworfene_ereignisse() > 0);
        assert_eq!(handle.zustand().await, PluginStatus::Degraded);
    }
}
