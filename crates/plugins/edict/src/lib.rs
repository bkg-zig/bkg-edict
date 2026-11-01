//! Edict als stdio-Plugin.
//!
//! Das Plugin bringt den mehrstufigen Ablauf des alten Systems in den
//! Host. Zwei Teile sind wichtig:
//!
//! - Die Haken geben dem Agenten zu Beginn, nach einem Werkzeugaufruf und
//!   am Ende einer Sitzung eine Anleitung.
//! - Die Werkzeugliste nennt die Begleitwerkzeuge und ihren Zustand.
//!
//! Das Plugin ruft Fähigkeiten des Hosts auf, etwa `agent.dispatch`, und
//! meldet das Ergebnis als Ereignis zurück. Der Host legt Kennung und
//! Zeitstempel des Ereignisses fest.

pub mod haken;
pub mod rpc;
pub mod werkzeuge;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use edict_core::topics;
use edict_plugin_protocol::{
    method, HealthResult, InitializeResult, InvokeResult, PluginState, RpcError, PROTOCOL_VERSION,
    RPC_INVALID_PARAMS, RPC_METHOD_NOT_FOUND,
};
use serde_json::{json, Value};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::RwLock;

use crate::haken::{finde, Haken};
use crate::rpc::{PluginHandler, RpcPeer};

/// Name des Plugins.
pub const PLUGIN_NAME: &str = "edict";

/// Fähigkeiten, die das Plugin anbietet.
pub const FAEHIGKEITEN: [&str; 2] = ["edict.hook.guidance", "edict.tool.status"];

/// Ereignisarten, die das Plugin abonniert.
///
/// Das Thema entspricht dem Thema des Auftragsereignisses in `edict-core`.
pub const ABONNEMENTS: [&str; 1] = [topics::TASK_DISPATCH];

/// Fähigkeit des Hosts, die das Plugin für einen Auftrag aufruft.
pub const HOST_FAEHIGKEIT: &str = "agent.dispatch";

/// Frist für einen Aufruf an den Host.
const AUFTRAG_FRIST: Duration = Duration::from_secs(60);

/// Zustand des Plugins.
pub struct EdictPlugin {
    gestartet: Instant,
    letzter_fehler: RwLock<Option<String>>,
    verworfen: AtomicU64,
    zustand: RwLock<PluginState>,
}

impl Default for EdictPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl EdictPlugin {
    /// Legt das Plugin an.
    pub fn new() -> Self {
        Self {
            gestartet: Instant::now(),
            letzter_fehler: RwLock::new(None),
            verworfen: AtomicU64::new(0),
            zustand: RwLock::new(PluginState::Ready),
        }
    }

    /// Baut die Anleitung eines Hakens.
    async fn guidance(&self, params: &Value) -> Result<Value, RpcError> {
        let name = params
            .get("hook")
            .and_then(Value::as_str)
            .ok_or_else(|| RpcError {
                code: RPC_INVALID_PARAMS,
                message: "Aufruf ohne Hakennamen".to_string(),
                data: None,
            })?;
        let Some(haken) = finde(name) else {
            return Err(RpcError {
                code: RPC_METHOD_NOT_FOUND,
                message: format!("Der Ablauf kennt den Haken {name} nicht"),
                data: None,
            });
        };
        let kanal = params.get("channel").and_then(Value::as_str);
        let agent = params.get("agent").and_then(Value::as_str);
        let text = haken::anleitung(haken, kanal, agent);
        Ok(json!({ "hook": haken.name, "guidance": text }))
    }

    /// Meldet den Zustand der Begleitwerkzeuge.
    async fn tool_status(&self) -> Result<Value, RpcError> {
        let status = werkzeuge::pruefe_alle();
        let werte: Vec<Value> = status
            .iter()
            .map(|eintrag| {
                json!({
                    "name": eintrag.name,
                    "binary": eintrag.binary,
                    "present": eintrag.vorhanden,
                    "path": eintrag.pfad,
                })
            })
            .collect();
        Ok(json!({ "tools": werte }))
    }

    /// Bearbeitet einen Auftrag, den der Host geschickt hat.
    ///
    /// Das Plugin ruft die Fähigkeit `agent.dispatch` des Hosts auf und
    /// meldet die Ausgabe als Ereignis auf dem Thema `agent.thoughts`.
    async fn bearbeite_auftrag(
        &self,
        verbindung: &Arc<RpcPeer>,
        params: &Value,
        trace_id: &str,
    ) -> Result<(), RpcError> {
        let task_id = params
            .get("task_id")
            .and_then(Value::as_str)
            .ok_or_else(|| RpcError {
                code: RPC_INVALID_PARAMS,
                message: "Auftrag ohne Aufgabenkennung".to_string(),
                data: None,
            })?
            .to_string();
        let agent = params
            .get("agent")
            .and_then(Value::as_str)
            .unwrap_or("unbekannt")
            .to_string();
        let message = params
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();

        let antwort = verbindung
            .ruf(
                method::INVOKE,
                json!({
                    "capability": HOST_FAEHIGKEIT,
                    "params": {
                        "task_id": task_id,
                        "agent": agent,
                        "message": message,
                    },
                }),
                AUFTRAG_FRIST,
            )
            .await?;

        let wert = antwort.get("value").cloned().unwrap_or(Value::Null);
        let meldung = json!({
            "topic": topics::AGENT_THOUGHTS,
            "event_type": "agent.output",
            "producer": "edict",
            "trace_id": trace_id,
            "payload": {
                "task_id": task_id,
                "agent": agent,
                "output": wert,
            },
        });
        verbindung
            .ruf(method::EMIT_EVENT, meldung, AUFTRAG_FRIST)
            .await?;
        Ok(())
    }

    /// Bearbeitet ein Ereignis, das der Host geschickt hat.
    ///
    /// Nur Aufträge des Themas `task.dispatch` werden bearbeitet. Andere
    /// Ereignisse werden ignoriert.
    ///
    /// Der Host schickt das ganze Ereignis. Die Nutzlast des Auftrags steht
    /// im Feld `payload`, die Spur des Vorgangs im Feld `trace_id`.
    async fn bearbeite_ereignis(
        &self,
        verbindung: &Arc<RpcPeer>,
        params: &Value,
    ) -> Result<(), RpcError> {
        let topic = params
            .get("topic")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if topic != topics::TASK_DISPATCH {
            return Ok(());
        }
        let nutzlast = params
            .get("payload")
            .cloned()
            .unwrap_or_else(|| params.clone());
        let trace_id = params
            .get("trace_id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        self.bearbeite_auftrag(verbindung, &nutzlast, trace_id)
            .await
    }
}

#[async_trait::async_trait]
impl PluginHandler for EdictPlugin {
    async fn bearbeite(
        &self,
        verbindung: &Arc<RpcPeer>,
        methode: &str,
        params: Value,
    ) -> Result<Value, RpcError> {
        match methode {
            method::INITIALIZE => {
                let version = params
                    .get("protocol_version")
                    .and_then(Value::as_u64)
                    .unwrap_or_default();
                if version != u64::from(PROTOCOL_VERSION) {
                    return Err(RpcError {
                        code: RPC_INVALID_PARAMS,
                        message: format!(
                            "Protokollversion {version} wird nicht unterstützt, erwartet wird {PROTOCOL_VERSION}"
                        ),
                        data: None,
                    });
                }
                let ergebnis = InitializeResult {
                    protocol_version: PROTOCOL_VERSION,
                    name: PLUGIN_NAME.to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                    capabilities: FAEHIGKEITEN.iter().map(|name| name.to_string()).collect(),
                    subscriptions: ABONNEMENTS.iter().map(|name| name.to_string()).collect(),
                    http_base_url: None,
                };
                Ok(serde_json::to_value(ergebnis).unwrap_or(Value::Null))
            }
            method::HEALTH => {
                let ergebnis = HealthResult {
                    state: *self.zustand.read().await,
                    uptime_seconds: self.gestartet.elapsed().as_secs(),
                    queued_events: 0,
                    dropped_events: self.verworfen.load(Ordering::SeqCst),
                    last_error: self.letzter_fehler.read().await.clone(),
                };
                Ok(serde_json::to_value(ergebnis).unwrap_or(Value::Null))
            }
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
                let wert = match capability.as_str() {
                    "edict.hook.guidance" => self.guidance(&aufruf_params).await?,
                    "edict.tool.status" => self.tool_status().await?,
                    other => {
                        return Err(RpcError {
                            code: RPC_METHOD_NOT_FOUND,
                            message: format!("edict kennt die Fähigkeit {other} nicht"),
                            data: None,
                        })
                    }
                };
                let ergebnis = InvokeResult { value: wert };
                Ok(serde_json::to_value(ergebnis).unwrap_or(Value::Null))
            }
            method::SHUTDOWN => {
                *self.zustand.write().await = PluginState::Failed;
                verbindung.beenden();
                Ok(json!({ "state": "beendet" }))
            }
            other => Err(rpc::unbekannte_methode(other)),
        }
    }

    /// Bearbeitet ein Ereignis, das der Host als Hinweis geschickt hat.
    async fn hinweis(&self, verbindung: &Arc<RpcPeer>, methode: &str, params: Value) {
        if methode != method::EVENT {
            return;
        }
        match self.bearbeite_ereignis(verbindung, &params).await {
            Ok(()) => {
                *self.zustand.write().await = PluginState::Ready;
            }
            Err(fehler) => {
                tracing::warn!("Auftrag konnte nicht bearbeitet werden: {}", fehler.message);
                *self.zustand.write().await = PluginState::Degraded;
                *self.letzter_fehler.write().await = Some(fehler.message);
            }
        }
    }
}

/// Bedient die Verbindung zum Host.
pub async fn serve<R, W>(leser: R, schreiber: W, plugin: Arc<EdictPlugin>)
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Send + Unpin + 'static,
{
    let verbindung = RpcPeer::new(Box::new(schreiber));
    rpc::bediene(tokio::io::BufReader::new(leser), verbindung, plugin).await;
}

/// Rechnet einen Haken in seinen Namen um.
#[must_use]
pub fn haken_name(haken: &Haken) -> &str {
    haken.name
}

#[cfg(test)]
mod tests {
    use super::*;
    use edict_plugin_protocol::{read_message, write_message, Message};

    /// Seite des Hosts im Protokollgespräch.
    ///
    /// Host und Plugin teilen sich eine Duplex-Pipe. Beide Richtungen laufen
    /// über getrennte Hälften, damit der Host während einer Anfrage des
    /// Plugins antworten kann.
    struct HostSeite {
        leser: tokio::io::BufReader<tokio::io::ReadHalf<tokio::io::DuplexStream>>,
        schreiber: tokio::io::WriteHalf<tokio::io::DuplexStream>,
    }

    impl HostSeite {
        /// Sendet eine Nachricht an das Plugin.
        async fn sende(&mut self, nachricht: &Message) {
            write_message(&mut self.schreiber, nachricht).await.unwrap();
        }

        /// Liest die nächste Nachricht vom Plugin.
        async fn lese(&mut self) -> Message {
            read_message(&mut self.leser).await.unwrap()
        }
    }

    /// Führt ein Protokollgespräch über eine Duplex-Pipe.
    async fn gespraech(plugin: Arc<EdictPlugin>) -> HostSeite {
        let (host_seite, plugin_seite) = tokio::io::duplex(64 * 1024);
        let (plugin_leser, plugin_schreiber) = tokio::io::split(plugin_seite);
        tokio::spawn(async move {
            serve(plugin_leser, plugin_schreiber, plugin).await;
        });
        let (host_leser, host_schreiber) = tokio::io::split(host_seite);
        HostSeite {
            leser: tokio::io::BufReader::new(host_leser),
            schreiber: host_schreiber,
        }
    }

    #[tokio::test]
    async fn handshake_nennt_faehigkeiten_und_abonnements() {
        let plugin = Arc::new(EdictPlugin::new());
        let mut host = gespraech(plugin).await;

        let anfrage = Message::request(
            Value::from(1),
            method::INITIALIZE,
            json!({"protocol_version": PROTOCOL_VERSION, "config": {}}),
        );
        host.sende(&anfrage).await;
        let antwort = host.lese().await;

        let info: InitializeResult =
            serde_json::from_value(antwort.result.expect("Ergebnis erwartet")).unwrap();
        assert_eq!(info.name, PLUGIN_NAME);
        assert_eq!(info.protocol_version, PROTOCOL_VERSION);
        assert_eq!(info.capabilities.len(), FAEHIGKEITEN.len());
        assert_eq!(info.subscriptions, vec![topics::TASK_DISPATCH.to_string()]);
    }

    #[tokio::test]
    async fn falsche_protokollversion_wird_abgewiesen() {
        let plugin = Arc::new(EdictPlugin::new());
        let mut host = gespraech(plugin).await;

        let anfrage = Message::request(
            Value::from(1),
            method::INITIALIZE,
            json!({"protocol_version": 99, "config": {}}),
        );
        host.sende(&anfrage).await;
        let antwort = host.lese().await;
        let fehler = antwort.error.expect("Fehler erwartet");
        assert_eq!(fehler.code, RPC_INVALID_PARAMS);
    }

    #[tokio::test]
    async fn gesundheit_wird_gemeldet() {
        let plugin = Arc::new(EdictPlugin::new());
        let mut host = gespraech(plugin).await;

        let anfrage = Message::request(Value::from(1), method::HEALTH, json!({}));
        host.sende(&anfrage).await;
        let antwort = host.lese().await;
        let gesundheit: HealthResult =
            serde_json::from_value(antwort.result.expect("Ergebnis erwartet")).unwrap();
        assert_eq!(gesundheit.state, PluginState::Ready);
    }

    #[tokio::test]
    async fn anleitung_und_werkzeugstatus_werden_geliefert() {
        let plugin = Arc::new(EdictPlugin::new());
        let mut host = gespraech(plugin).await;

        let anfrage = Message::request(
            Value::from(1),
            method::INVOKE,
            json!({
                "capability": "edict.hook.guidance",
                "params": {"hook": "session-start", "channel": "projekt"},
            }),
        );
        host.sende(&anfrage).await;
        let antwort = host.lese().await;
        let ergebnis: InvokeResult =
            serde_json::from_value(antwort.result.expect("Ergebnis erwartet")).unwrap();
        assert!(ergebnis.value["guidance"]
            .as_str()
            .unwrap()
            .contains("session-start"));

        let anfrage = Message::request(
            Value::from(2),
            method::INVOKE,
            json!({
                "capability": "edict.tool.status",
                "params": {},
            }),
        );
        host.sende(&anfrage).await;
        let antwort = host.lese().await;
        let ergebnis: InvokeResult =
            serde_json::from_value(antwort.result.expect("Ergebnis erwartet")).unwrap();
        assert_eq!(
            ergebnis.value["tools"].as_array().unwrap().len(),
            werkzeuge::WERKZEUGE.len()
        );
    }

    #[tokio::test]
    async fn unbekannte_faehigkeit_wird_abgewiesen() {
        let plugin = Arc::new(EdictPlugin::new());
        let mut host = gespraech(plugin).await;

        let anfrage = Message::request(
            Value::from(1),
            method::INVOKE,
            json!({"capability": "gibt-es-nicht", "params": {}}),
        );
        host.sende(&anfrage).await;
        let antwort = host.lese().await;
        let fehler = antwort.error.expect("Fehler erwartet");
        assert_eq!(fehler.code, RPC_METHOD_NOT_FOUND);
    }

    #[tokio::test]
    async fn auftrag_ruft_den_host_und_meldet_das_ergebnis() {
        let plugin = Arc::new(EdictPlugin::new());
        let mut host = gespraech(plugin).await;

        // Das Plugin meldet den Auftrag als Hinweis. Danach ruft es den Host
        // auf und meldet das Ergebnis zurück.
        let ereignis = json!({
            "topic": topics::TASK_DISPATCH,
            "event_type": "task.dispatch.request",
            "producer": "task_service",
            "trace_id": "spur-1",
            "payload": {
                "task_id": "JJC-1",
                "agent": "zhongshu",
                "message": "Prüfe die Änderung",
            },
        });
        let hinweis = Message::notification(method::EVENT, ereignis);
        host.sende(&hinweis).await;

        // Der Host beantwortet den Aufruf des Plugins.
        let anfrage = host.lese().await;
        assert_eq!(anfrage.method.as_deref(), Some(method::INVOKE));
        assert_eq!(anfrage.params.unwrap()["capability"], HOST_FAEHIGKEIT);
        let kennung = anfrage.id.expect("Kennung erwartet");
        let antwort = Message::response(
            kennung,
            json!({"value": {"output": "Ergebnis der Prüfung"}}),
        );
        host.sende(&antwort).await;

        // Das Plugin meldet das Ergebnis als Ereignis.
        let meldung = host.lese().await;
        assert_eq!(meldung.method.as_deref(), Some(method::EMIT_EVENT));
        let kennung = meldung.id.expect("Kennung erwartet");
        let params = meldung.params.expect("Parameter erwartet");
        host.sende(&Message::response(kennung, json!({"event_id": "abc"})))
            .await;

        assert_eq!(params["topic"], topics::AGENT_THOUGHTS);
        assert_eq!(params["trace_id"], "spur-1");
        assert_eq!(params["payload"]["task_id"], "JJC-1");
        assert_eq!(
            params["payload"]["output"]["output"],
            "Ergebnis der Prüfung"
        );
    }

    #[tokio::test]
    async fn beenden_beendet_die_verbindung() {
        let plugin = Arc::new(EdictPlugin::new());
        let mut host = gespraech(plugin).await;

        let anfrage = Message::request(Value::from(1), method::SHUTDOWN, json!({}));
        host.sende(&anfrage).await;
        let antwort = host.lese().await;
        assert!(antwort.result.is_some());
    }
}
