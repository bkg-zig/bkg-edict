//! Verbindung eines Plugins zum Host.
//!
//! Das Plugin liest Anfragen des Hosts auf stdin und schreibt Antworten auf
//! stdout. Beide Richtungen laufen über denselben Rahmen. Das Plugin kann
//! selbst Anfragen an den Host senden, etwa `invoke` oder `emit_event`.
//!
//! Auf stdout gehören ausschließlich Rahmen. Jede andere Ausgabe wäre ein
//! Defekt im Plugin.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use edict_plugin_protocol::{read_message, write_message, Message, RpcError, RPC_METHOD_NOT_FOUND};
use serde_json::Value;
use tokio::io::{AsyncRead, AsyncWrite, BufReader};
use tokio::sync::{oneshot, Mutex};

/// Schreibende Seite der Verbindung.
pub type Schreibende = Box<dyn AsyncWrite + Send + Unpin>;

/// Behandler für Anfragen, die der Host an das Plugin richtet.
#[async_trait]
pub trait PluginHandler: Send + Sync {
    /// Beantwortet eine Anfrage des Hosts.
    async fn bearbeite(
        &self,
        verbindung: &Arc<RpcPeer>,
        methode: &str,
        params: Value,
    ) -> Result<Value, RpcError>;

    /// Bearbeitet einen Hinweis des Hosts.
    ///
    /// Der Host schickt Ereignisse ohne Kennung, weil er keine Antwort
    /// erwartet. Die Task läuft getrennt von der Leseschleife, damit ein
    /// Hinweis die Verbindung nicht blockiert.
    async fn hinweis(&self, verbindung: &Arc<RpcPeer>, methode: &str, params: Value) {
        let _ = (verbindung, methode, params);
    }
}

/// Verbindung eines Plugins zum Host.
pub struct RpcPeer {
    schreiber: Arc<Mutex<Schreibende>>,
    offen: Mutex<HashMap<u64, oneshot::Sender<Result<Value, RpcError>>>>,
    naechste: AtomicU64,
    beendet: AtomicBool,
}

impl RpcPeer {
    /// Legt eine Verbindung an.
    pub fn new(schreiber: Schreibende) -> Arc<Self> {
        Arc::new(Self {
            schreiber: Arc::new(Mutex::new(schreiber)),
            offen: Mutex::new(HashMap::new()),
            naechste: AtomicU64::new(1),
            beendet: AtomicBool::new(false),
        })
    }

    /// Sendet eine Anfrage an den Host und wartet auf die Antwort.
    pub async fn ruf(
        &self,
        methode: &str,
        params: Value,
        frist: Duration,
    ) -> Result<Value, RpcError> {
        let kennung = self.naechste.fetch_add(1, Ordering::SeqCst);
        let (sender, empfaenger) = oneshot::channel();
        self.offen.lock().await.insert(kennung, sender);
        let anfrage = Message::request(Value::from(kennung), methode, params);
        if let Err(fehler) = self.schreibe(&anfrage).await {
            self.offen.lock().await.remove(&kennung);
            return Err(fehler);
        }

        match tokio::time::timeout(frist, empfaenger).await {
            Ok(Ok(antwort)) => antwort,
            Ok(Err(_)) => {
                self.offen.lock().await.remove(&kennung);
                Err(RpcError {
                    code: -32000,
                    message: format!("Der Host hat die Antwort auf {methode} nie geschickt"),
                    data: None,
                })
            }
            Err(_) => {
                self.offen.lock().await.remove(&kennung);
                Err(RpcError {
                    code: -32000,
                    message: format!("Zeitüberschreitung nach {frist:?} bei {methode}"),
                    data: None,
                })
            }
        }
    }

    /// Sendet einen Hinweis ohne Antwort an den Host.
    pub async fn hinweis(&self, methode: &str, params: Value) -> Result<(), RpcError> {
        let nachricht = Message::notification(methode, params);
        self.schreibe(&nachricht).await
    }

    /// Schreibt einen Rahmen an den Host.
    async fn schreibe(&self, nachricht: &Message) -> Result<(), RpcError> {
        let mut schreiber = self.schreiber.lock().await;
        write_message(&mut *schreiber, nachricht)
            .await
            .map_err(|fehler| RpcError {
                code: -32000,
                message: format!("Schreiben fehlgeschlagen: {fehler}"),
                data: None,
            })
    }

    /// Meldet, ob das Plugin den Betrieb beenden soll.
    pub fn beendet(&self) -> bool {
        self.beendet.load(Ordering::SeqCst)
    }

    /// Fordert das Ende der Verbindung an.
    pub fn beenden(&self) {
        self.beendet.store(true, Ordering::SeqCst);
    }
}

/// Bedient die Verbindung, bis der Host das Plugin beendet.
///
/// Die Schleife beantwortet Anfragen des Hosts und löst Antworten auf eigene
/// Anfragen auf. Jede Anfrage des Hosts läuft in einer eigenen Task. Sonst
/// könnte das Plugin keine Anfrage an den Host stellen, während es eine
/// Anfrage des Hosts beantwortet.
pub async fn bediene<L>(
    mut leser: BufReader<L>,
    verbindung: Arc<RpcPeer>,
    handler: Arc<dyn PluginHandler>,
) where
    L: AsyncRead + Unpin,
{
    loop {
        let nachricht = match read_message(&mut leser).await {
            Ok(nachricht) => nachricht,
            Err(fehler) => {
                tracing::warn!("Verbindung zum Host beendet: {fehler}");
                return;
            }
        };

        match (nachricht.id.clone(), nachricht.method.clone()) {
            (Some(kennung), Some(methode)) => {
                let params = nachricht.params.clone().unwrap_or(Value::Null);
                let verbindungs_wg = verbindung.clone();
                let handler_wg = handler.clone();
                tokio::spawn(async move {
                    let antwort = match handler_wg
                        .bearbeite(&verbindungs_wg, &methode, params)
                        .await
                    {
                        Ok(wert) => Message::response(kennung, wert),
                        Err(fehler) => {
                            Message::error_response(kennung, fehler.code, fehler.message)
                        }
                    };
                    if let Err(fehler) = verbindungs_wg.schreibe(&antwort).await {
                        tracing::warn!("Antwort konnte nicht gesendet werden: {}", fehler.message);
                    }
                });
            }
            (Some(kennung), None) => {
                let antwort = match (nachricht.result, nachricht.error) {
                    (_, Some(fehler)) => Err(fehler),
                    (Some(wert), None) => Ok(wert),
                    (None, None) => Err(RpcError {
                        code: -32602,
                        message: "Antwort ohne Ergebnis und ohne Fehler".to_string(),
                        data: None,
                    }),
                };
                let nummer = kennung.as_u64().unwrap_or_default();
                let wartekanal = verbindung.offen.lock().await.remove(&nummer);
                if let Some(wartekanal) = wartekanal {
                    let _ = wartekanal.send(antwort);
                }
            }
            (None, Some(methode)) => {
                let params = nachricht.params.clone().unwrap_or(Value::Null);
                let verbindungs_wg = verbindung.clone();
                let handler_wg = handler.clone();
                tokio::spawn(async move {
                    handler_wg.hinweis(&verbindungs_wg, &methode, params).await;
                });
            }
            (None, None) => {}
        }
    }
}

/// Antwortet auf eine unbekannte Methode.
pub fn unbekannte_methode(methode: &str) -> RpcError {
    RpcError {
        code: RPC_METHOD_NOT_FOUND,
        message: format!("Das Plugin kennt die Methode {methode} nicht"),
        data: None,
    }
}
