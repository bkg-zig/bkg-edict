//! Ein einzelnes Plugin im Host.
//!
//! Der Griff hält Manifest, Prozess, Zustand und Ereigniswarteschlange.
//! Er übernimmt den Handshake, die Zustellung von Ereignissen, Aufrufe in
//! beide Richtungen und das geordnete Beenden.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::Utc;
use edict_core::{topics, AppError, AppResult, Event};
use edict_plugin_protocol::{
    method, HealthResult, InitializeResult, InvokeResult, RpcError, PROTOCOL_VERSION,
    RPC_INVALID_PARAMS, RPC_METHOD_NOT_FOUND, RPC_PLUGIN_ERROR,
};
use serde_json::{json, Value};
use tokio::sync::{mpsc, Notify, RwLock};

use crate::env::build_plugin_env;
use crate::manifest::Manifest;
use crate::services::{EventSink, HostServices};
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
    #[must_use]
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
    pub services: Arc<dyn HostServices>,
    /// Ablage für Domainereignisse.
    pub sink: Arc<dyn EventSink>,
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
                    Ok(wert) => Ok(json!({ "value": wert })),
                    Err(fehler) => Err(RpcError {
                        code: RPC_PLUGIN_ERROR,
                        message: fehler.to_string(),
                        data: None,
                    }),
                }
            }
            method::EMIT_EVENT => {
                let ereignis = ereignis_aus_params(&params)?;
                match self.sink.append_event(&ereignis).await {
                    Ok(()) => Ok(json!({ "event_id": ereignis.event_id })),
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

/// Baut ein Domainereignis aus den Angaben eines Plugins.
///
/// Der Host vergibt Kennung und Zeitstempel. Das Plugin darf nur Thema,
/// Ereignisart, Erzeuger, Trace und Nutzlast bestimmen.
fn ereignis_aus_params(params: &Value) -> Result<Event, RpcError> {
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

    Ok(Event {
        event_id: uuid::Uuid::new_v4().to_string(),
        trace_id,
        timestamp: Utc::now().to_rfc3339(),
        topic,
        event_type,
        producer,
        payload,
        meta,
    })
}

/// Baut die Parameter eines Hinweises, mit dem der Host ein Ereignis an ein
/// Plugin schickt.
///
/// Das Plugin erhält das ganze Ereignis. Ohne Thema und Ereignisart könnte
/// es nicht entscheiden, ob der Hinweis zu ihm gehört.
fn ereignis_als_params(ereignis: &Event) -> Value {
    serde_json::to_value(ereignis).unwrap_or(Value::Null)
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
    prozess_gewechselt: Notify,
    sender: mpsc::Sender<Event>,
    wartende: AtomicU64,
    verworfen: AtomicU64,
    handshake_fertig: AtomicBool,
    letzter_fehler: RwLock<Option<String>>,
    letzter_code: RwLock<Option<i32>>,
    gestartet: RwLock<Option<Instant>>,
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
            prozess_gewechselt: Notify::new(),
            sender,
            wartende: AtomicU64::new(0),
            verworfen: AtomicU64::new(0),
            handshake_fertig: AtomicBool::new(false),
            letzter_fehler: RwLock::new(None),
            letzter_code: RwLock::new(None),
            gestartet: RwLock::new(None),
        });
        handle.starte_zusteller(receiver);
        handle
    }

    /// Startet die Task, die die Warteschlange leer macht.
    ///
    /// Die Task wartet zuerst auf einen laufenden Prozess. Solange keiner
    /// läuft, bleibt die Warteschlange stehen und kann sich füllen.
    fn starte_zusteller(self: &Arc<Self>, mut receiver: mpsc::Receiver<Event>) {
        let handle = self.clone();
        tokio::spawn(async move {
            loop {
                let prozess = handle.warte_auf_prozess().await;
                let Some(prozess) = prozess else {
                    break;
                };
                let ereignis = match receiver.recv().await {
                    Some(ereignis) => ereignis,
                    None => break,
                };
                handle.wartende.fetch_sub(1, Ordering::SeqCst);
                let params = ereignis_als_params(&ereignis);
                match prozess.hinweis(method::EVENT, params).await {
                    Ok(()) => {
                        let mut zustand = handle.zustand.write().await;
                        if *zustand == PluginStatus::Degraded
                            && handle.wartende.load(Ordering::SeqCst)
                                < handle.config.event_queue_capacity as u64
                        {
                            *zustand = PluginStatus::Ready;
                        }
                    }
                    Err(fehler) => {
                        tracing::warn!(
                            plugin = %handle.manifest.name,
                            "Ereignis konnte nicht gesendet werden: {fehler}"
                        );
                        handle.verworfen.fetch_add(1, Ordering::SeqCst);
                        handle
                            .letzter_fehler
                            .write()
                            .await
                            .replace(fehler.to_string());
                    }
                }
            }
        });
    }

    /// Wartet, bis ein Prozess läuft.
    async fn warte_auf_prozess(&self) -> Option<Arc<PluginProcess>> {
        loop {
            if let Some(prozess) = self.prozess.read().await.clone() {
                return Some(prozess);
            }
            let wartender = self.prozess_gewechselt.notified();
            if let Some(prozess) = self.prozess.read().await.clone() {
                return Some(prozess);
            }
            wartender.await;
        }
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
    ///
    /// Stimmt die Protokollversion des Plugins nicht mit dem Manifest überein,
    /// endet der Prozess mit dem Code [`RPC_INVALID_PARAMS`].
    pub async fn start(&self, bridge: Arc<HostBridge>) -> AppResult<()> {
        *self.zustand.write().await = PluginStatus::Starting;
        *self.letzter_fehler.write().await = None;
        *self.letzter_code.write().await = None;
        self.handshake_fertig.store(false, Ordering::SeqCst);

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
        self.prozess_gewechselt.notify_waiters();

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

        if info.protocol_version != self.manifest.protocol_version
            || self.manifest.protocol_version != PROTOCOL_VERSION
        {
            self.beende_prozess().await;
            *self.zustand.write().await = PluginStatus::Failed;
            let text = format!(
                "Protokollversion des Plugins {} passt nicht zum Manifest {} und zur Version {PROTOCOL_VERSION}",
                info.protocol_version, self.manifest.protocol_version
            );
            *self.letzter_fehler.write().await = Some(text.clone());
            *self.letzter_code.write().await = Some(RPC_INVALID_PARAMS);
            return Err(AppError::Validation(text));
        }

        *self.info.write().await = info;
        self.handshake_fertig.store(true, Ordering::SeqCst);
        *self.gestartet.write().await = Some(Instant::now());
        *self.zustand.write().await = PluginStatus::Ready;
        Ok(())
    }

    /// Beendet den Kindprozess mit Frist und danach endgültig.
    ///
    /// Das Plugin bekommt zuerst die Methode `shutdown`. Läuft es danach
    /// noch, endet der Prozess.
    pub async fn stop(&self) -> AppResult<()> {
        let prozess = self.prozess.read().await.clone();
        if let Some(prozess) = prozess {
            let _ = prozess
                .ruf(method::SHUTDOWN, json!({}), self.config.shutdown_grace)
                .await;
            if !prozess.warte_aus(self.config.shutdown_grace).await {
                tracing::warn!(plugin = %self.manifest.name, "Plugin beendete sich nicht rechtzeitig");
            }
            prozess.beende().await;
        }
        *self.prozess.write().await = None;
        *self.gestartet.write().await = None;
        self.handshake_fertig.store(false, Ordering::SeqCst);
        *self.zustand.write().await = PluginStatus::Stopped;
        self.prozess_gewechselt.notify_waiters();
        Ok(())
    }

    /// Beendet den Kindprozess ohne Frist.
    async fn beende_prozess(&self) {
        if let Some(prozess) = self.prozess.read().await.clone() {
            prozess.beende().await;
        }
        *self.prozess.write().await = None;
        self.prozess_gewechselt.notify_waiters();
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
                json!({ "capability": capability, "params": params }),
                self.config.dispatch_timeout,
            )
            .await?;
        let ergebnis: InvokeResult = serde_json::from_value(antwort)?;
        Ok(ergebnis.value)
    }

    /// Fragt den Zustand des Plugins ab.
    pub async fn health(&self) -> AppResult<HealthResult> {
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
            Ok(()) => {
                self.wartende.fetch_add(1, Ordering::SeqCst);
                true
            }
            Err(mpsc::error::TrySendError::Full(_)) => {
                self.verworfen.fetch_add(1, Ordering::SeqCst);
                let mut zustand = self.zustand.write().await;
                if *zustand != PluginStatus::Failed && *zustand != PluginStatus::Stopped {
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
        let abonnements = self.abonnements();
        if abonnements.is_empty() {
            return false;
        }
        abonnements
            .iter()
            .any(|muster| muster == "*" || muster == topic)
    }

    /// Abonnierte Ereignisarten des Plugins.
    ///
    /// Vor dem Handshake gelten die Abonnements aus dem Manifest. Danach
    /// gelten die Abonnements, die das Plugin selbst gemeldet hat.
    pub fn abonnements(&self) -> Vec<String> {
        if !self.handshake_fertig.load(Ordering::SeqCst) {
            return self.manifest.subscriptions.clone();
        }
        self.info
            .try_read()
            .map(|info| info.subscriptions.clone())
            .unwrap_or_else(|_| self.manifest.subscriptions.clone())
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
            subscriptions: self.abonnements(),
            http_base_url: info.http_base_url.clone(),
            dropped_events: self.verworfen.load(Ordering::SeqCst),
            queued_events: self.wartende.load(Ordering::SeqCst),
            uptime_seconds,
            last_error: self.letzter_fehler.read().await.clone(),
        }
    }

    /// Anzahl der verworfenen Ereignisse.
    pub fn verworfene_ereignisse(&self) -> u64 {
        self.verworfen.load(Ordering::SeqCst)
    }

    /// JSON-RPC-Code des letzten Fehlers.
    pub async fn letzter_fehlercode(&self) -> Option<i32> {
        *self.letzter_code.read().await
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
#[must_use]
pub fn review_request_event(task_id: &str, dateien: &[String]) -> Event {
    let mut payload = serde_json::Map::new();
    payload.insert("task_id".to_string(), Value::String(task_id.to_string()));
    payload.insert(
        "files".to_string(),
        Value::Array(
            dateien
                .iter()
                .map(|name| Value::String(name.clone()))
                .collect(),
        ),
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
