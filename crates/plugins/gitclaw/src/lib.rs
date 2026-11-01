//! gitclaw als stdio-Plugin.
//!
//! Das Plugin nimmt Webhooks über einen kleinen HTTP-Server auf
//! `127.0.0.1` an. Für jeden Webhook prüft es die Signatur, ermittelt die
//! betroffenen Dateien, gibt einen Prüfauftrag an den Host und meldet das
//! Ergebnis zurück.
//!
//! Der Host spricht mit dem Plugin über JSON-RPC 2.0 auf stdio. Das Plugin
//! nutzt dafür den Rahmen aus `edict-plugin-protocol`.

pub mod review;
pub mod rpc;
pub mod signature;
pub mod webhook;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use edict_plugin_protocol::{
    method, HealthResult, InitializeResult, InvokeResult, PluginState, RpcError, PROTOCOL_VERSION,
    RPC_INVALID_PARAMS, RPC_METHOD_NOT_FOUND,
};
use serde_json::{json, Value};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::RwLock;

use rpc::{PluginHandler, RpcPeer};
use signature::{HmacSha256, KeinSchluessel, SignaturPruefer};
use webhook::WebhookZustand;

/// Name des Plugins.
pub const PLUGIN_NAME: &str = "gitclaw";

/// Fähigkeiten, die das Plugin anbietet.
pub const FAEHIGKEITEN: [&str; 2] = ["gitclaw.affected_files", "gitclaw.review"];

/// Ereignisarten, die das Plugin abonniert.
pub const ABONNEMENTS: [&str; 1] = ["task.review.request"];

/// Frist für einen Aufruf an den Host.
const AUFTRAG_FRIST: Duration = Duration::from_secs(30);

/// Zustand des Plugins.
pub struct GitclawPlugin {
    gestartet: Instant,
    http_basis: RwLock<Option<String>>,
    letzter_fehler: RwLock<Option<String>>,
    verworfen: AtomicU64,
    zustand: RwLock<PluginState>,
}

impl Default for GitclawPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl GitclawPlugin {
    /// Legt das Plugin an.
    pub fn new() -> Self {
        Self {
            gestartet: Instant::now(),
            http_basis: RwLock::new(None),
            letzter_fehler: RwLock::new(None),
            verworfen: AtomicU64::new(0),
            zustand: RwLock::new(PluginState::Ready),
        }
    }

    /// Adresse des HTTP-Eingangs.
    pub async fn http_basis(&self) -> Option<String> {
        self.http_basis.read().await.clone()
    }

    /// Liest den Signaturschlüssel.
    ///
    /// Der Schlüssel kommt aus der Konfiguration des Hosts. Fehlt er, liest
    /// das Plugin die Datei `gitclaw-secret` aus seinem Verzeichnis. `HOME`
    /// zeigt beim Start auf dieses Verzeichnis.
    async fn signaturschluessel(config: &Value) -> Option<Vec<u8>> {
        if let Some(wert) = config.get("webhook_secret").and_then(Value::as_str) {
            if !wert.trim().is_empty() {
                return Some(wert.trim().as_bytes().to_vec());
            }
        }
        let pfad = std::env::var("HOME").ok()?;
        let text =
            std::fs::read_to_string(std::path::Path::new(&pfad).join("gitclaw-secret")).ok()?;
        let wert = text.trim();
        if wert.is_empty() {
            None
        } else {
            Some(wert.as_bytes().to_vec())
        }
    }

    /// Startet den HTTP-Eingang und meldet die Adresse zurück.
    async fn starte_http(
        &self,
        verbindung: &Arc<RpcPeer>,
        config: &Value,
    ) -> Result<String, RpcError> {
        let port = config
            .get("http_port")
            .and_then(Value::as_u64)
            .unwrap_or_default();
        let port = u16::try_from(port).unwrap_or_default();
        let (adresse, listener) = webhook::binde(port).await.map_err(|fehler| RpcError {
            code: RPC_INVALID_PARAMS,
            message: format!("HTTP-Eingang nicht startbar: {fehler}"),
            data: None,
        })?;

        let schluessel = Self::signaturschluessel(config).await;
        let pruefer: Arc<dyn SignaturPruefer> = match schluessel {
            Some(schluessel) => Arc::new(HmacSha256::new(schluessel)),
            None => Arc::new(KeinSchluessel),
        };
        let zustand = Arc::new(WebhookZustand {
            verbindung: verbindung.clone(),
            pruefer,
        });

        tokio::spawn(async move {
            if let Err(fehler) = webhook::starte(listener, zustand).await {
                tracing::error!("HTTP-Eingang beendet: {fehler}");
            }
        });

        Ok(format!("http://{adresse}"))
    }

    /// Berechnet die betroffenen Dateien eines Ereignisses.
    async fn betroffene_dateien(&self, params: &Value) -> Result<Value, RpcError> {
        let Some(ereignis) = params.get("event").and_then(Value::as_str) else {
            return Err(RpcError {
                code: RPC_INVALID_PARAMS,
                message: "Aufruf ohne Ereignis".to_string(),
                data: None,
            });
        };
        let art =
            review::WebhookArt::aus_name(params.get("art").and_then(Value::as_str).unwrap_or(""));
        let Ok(wert) = serde_json::from_str::<Value>(ereignis) else {
            return Err(RpcError {
                code: RPC_INVALID_PARAMS,
                message: "Ereignis ist kein gültiges JSON".to_string(),
                data: None,
            });
        };
        let Some(objekt) = wert.as_object() else {
            return Err(RpcError {
                code: RPC_INVALID_PARAMS,
                message: "Ereignis ist kein JSON-Objekt".to_string(),
                data: None,
            });
        };
        let dateien = review::betroffene_dateien(art, objekt);
        Ok(json!({ "dateien": dateien }))
    }

    /// Gibt einen Prüfauftrag an den Host.
    ///
    /// Der Host führt die Prüfung aus. Das Plugin meldet danach das Ergebnis
    /// als Ereignis.
    async fn pruefe(&self, verbindung: &Arc<RpcPeer>, params: &Value) -> Result<Value, RpcError> {
        let auftrag = review::auftrag_nutzlast(&review::WebhookEreignis {
            art: review::WebhookArt::aus_name(
                params.get("art").and_then(Value::as_str).unwrap_or(""),
            ),
            lager: params
                .get("lager")
                .and_then(Value::as_str)
                .map(str::to_string),
            dateien: params
                .get("dateien")
                .and_then(Value::as_array)
                .map(|liste| {
                    liste
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default(),
            zusammenfassung: params
                .get("zusammenfassung")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
        });
        let ergebnis = verbindung
            .ruf(
                method::INVOKE,
                json!({
                    "capability": "review.request",
                    "params": auftrag,
                }),
                AUFTRAG_FRIST,
            )
            .await?;
        Ok(json!({ "value": ergebnis.get("value").cloned().unwrap_or(Value::Null) }))
    }
}

#[async_trait::async_trait]
impl PluginHandler for GitclawPlugin {
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
                let config = params.get("config").cloned().unwrap_or(json!({}));
                let basis = self.starte_http(verbindung, &config).await?;
                *self.http_basis.write().await = Some(basis.clone());
                let ergebnis = InitializeResult {
                    protocol_version: PROTOCOL_VERSION,
                    name: PLUGIN_NAME.to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                    capabilities: FAEHIGKEITEN.iter().map(|name| name.to_string()).collect(),
                    subscriptions: ABONNEMENTS.iter().map(|name| name.to_string()).collect(),
                    http_base_url: Some(basis),
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
                    "gitclaw.affected_files" => self.betroffene_dateien(&aufruf_params).await?,
                    "gitclaw.review" => self.pruefe(verbindung, &aufruf_params).await?,
                    other => {
                        return Err(RpcError {
                            code: RPC_METHOD_NOT_FOUND,
                            message: format!("gitclaw kennt die Fähigkeit {other} nicht"),
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
            method::EVENT => {
                *self.zustand.write().await = PluginState::Ready;
                Ok(json!({ "state": "ready" }))
            }
            other => Err(rpc::unbekannte_methode(other)),
        }
    }
}

/// Bedient die Verbindung zum Host.
pub async fn serve<R, W>(leser: R, schreiber: W, plugin: Arc<GitclawPlugin>)
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Send + Unpin + 'static,
{
    let verbindung = RpcPeer::new(Box::new(schreiber));
    rpc::bediene(tokio::io::BufReader::new(leser), verbindung, plugin).await;
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
    async fn gespraech(plugin: Arc<GitclawPlugin>) -> HostSeite {
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
    async fn handshake_und_status_sind_vollstaendig() {
        let plugin = Arc::new(GitclawPlugin::new());
        let mut host = gespraech(plugin.clone()).await;

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
        assert!(info.http_base_url.is_some());
        assert_eq!(info.capabilities.len(), FAEHIGKEITEN.len());
    }

    #[tokio::test]
    async fn falsche_protokollversion_wird_abgewiesen() {
        let plugin = Arc::new(GitclawPlugin::new());
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
        let plugin = Arc::new(GitclawPlugin::new());
        let mut host = gespraech(plugin).await;

        let anfrage = Message::request(Value::from(1), method::HEALTH, json!({}));
        host.sende(&anfrage).await;
        let antwort = host.lese().await;
        let gesundheit: HealthResult =
            serde_json::from_value(antwort.result.expect("Ergebnis erwartet")).unwrap();
        assert_eq!(gesundheit.state, PluginState::Ready);
    }
}
