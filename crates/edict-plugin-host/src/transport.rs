//! Prozessstart und JSON-RPC-Verbindung zu einem Plugin.
//!
//! Der Host startet den Kindprozess ohne Shell. Die Umgebung des Kindes
//! enthält nur `PATH`, `LANG` und `HOME`. Das Kommando wird ausschließlich
//! im Pluginverzeichnis aufgelöst.
//!
//! Auf stdout des Plugins gehören nur Rahmen. Jede andere Ausgabe ist ein
//! Defekt im Plugin und beendet den Prozess. stderr wird als Protokoll
//! gelesen.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use edict_core::{AppError, AppResult};
use edict_plugin_protocol::{
    read_message, write_message, Message, RpcError, RPC_INVALID_PARAMS, RPC_METHOD_NOT_FOUND,
    RPC_PLUGIN_ERROR,
};
use serde_json::Value;
use std::process::Stdio;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{oneshot, Mutex};

/// Anfragen des Plugins an den Host.
///
/// Ein Plugin darf den Host um Fähigkeiten bitten, etwa `task.lookup`, und
/// Domainereignisse melden. Der Host beantwortet beide Richtungen.
#[async_trait]
pub trait HostRequestHandler: Send + Sync {
    /// Beantwortet eine Anfrage des Plugins.
    async fn handle(&self, method: &str, params: Value) -> Result<Value, RpcError>;
}

/// Laufender Kindprozess eines Plugins.
///
/// Die Struktur hält den Prozess, den Schreibkanal und die offenen Anfragen.
/// Der Lesevorgang läuft in einer eigenen Task und beantwortet Anfragen des
/// Plugins.
pub struct PluginProcess {
    kind: Mutex<Child>,
    stdin: Mutex<ChildStdin>,
    offen: Mutex<BTreeMap<u64, oneshot::Sender<Result<Value, RpcError>>>>,
    naechste_kennung: AtomicU64,
}

impl PluginProcess {
    /// Startet den Kindprozess.
    ///
    /// `umgebung` enthält die vollständige Umgebung des Kindes. Der Host
    /// erzeugt sie mit [`crate::env::build_plugin_env`].
    pub async fn start(
        kommando: &Path,
        arbeitsverzeichnis: &Path,
        umgebung: &BTreeMap<String, String>,
        handler: Arc<dyn HostRequestHandler>,
    ) -> AppResult<Arc<Self>> {
        let mut befehl = Command::new(kommando);
        befehl
            .current_dir(arbeitsverzeichnis)
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        for (name, wert) in umgebung {
            befehl.env(name, wert);
        }
        eigene_prozessgruppe(&mut befehl);

        let mut kind = befehl.spawn().map_err(|fehler| {
            AppError::Io(std::io::Error::new(
                fehler.kind(),
                format!("Pluginprozess {kommando:?} startete nicht: {fehler}"),
            ))
        })?;
        let stdin = kind.stdin.take().ok_or_else(|| {
            AppError::Internal("Pluginprozess bekam keinen Schreibkanal".to_string())
        })?;
        let stdout = kind.stdout.take().ok_or_else(|| {
            AppError::Internal("Pluginprozess bekam keinen Lesekanal".to_string())
        })?;
        let stderr = kind.stderr.take().ok_or_else(|| {
            AppError::Internal("Pluginprozess bekam keinen Fehlerkanal".to_string())
        })?;

        let prozess = Arc::new(Self {
            kind: Mutex::new(kind),
            stdin: Mutex::new(stdin),
            offen: Mutex::new(BTreeMap::new()),
            naechste_kennung: AtomicU64::new(1),
        });

        let protokoll_task = prozess.clone();
        tokio::spawn(async move {
            protokoll_task.lese_aufgaben(stdout, handler).await;
        });

        let fehler_task = prozess.clone();
        tokio::spawn(async move {
            fehler_task.lese_fehler(stderr).await;
        });

        Ok(prozess)
    }

    /// Liest stderr als Protokoll.
    async fn lese_fehler(self: Arc<Self>, stderr: tokio::process::ChildStderr) {
        let mut zeilen = BufReader::new(stderr).lines();
        while let Ok(Some(zeile)) = zeilen.next_line().await {
            tracing::warn!(plugin = "stderr", "{zeile}");
        }
    }

    /// Liest Rahmen von stdout und beantwortet das Plugin.
    ///
    /// Ein Fehler im Rahmen bricht die Verbindung ab. Das Plugin hat dann
    /// etwas anderes als Protokoll auf stdout geschrieben.
    async fn lese_aufgaben(
        self: Arc<Self>,
        stdout: tokio::process::ChildStdout,
        handler: Arc<dyn HostRequestHandler>,
    ) {
        let mut leser = BufReader::new(stdout);
        loop {
            let nachricht = match read_message(&mut leser).await {
                Ok(nachricht) => nachricht,
                Err(fehler) => {
                    tracing::error!(plugin = "stdout", "Protokollfehler: {fehler}");
                    self.beende().await;
                    return;
                }
            };
            self.verarbeite(nachricht, handler.clone()).await;
        }
    }

    /// Verteilt eine eingegangene Nachricht.
    async fn verarbeite(&self, nachricht: Message, handler: Arc<dyn HostRequestHandler>) {
        match (nachricht.id.clone(), nachricht.method.clone()) {
            (Some(kennung), Some(methode)) => {
                let params = nachricht.params.clone().unwrap_or(Value::Null);
                let antwort = match handler.handle(&methode, params).await {
                    Ok(wert) => Message::response(kennung, wert),
                    Err(fehler) => Message::error_response(kennung, fehler.code, fehler.message),
                };
                if let Err(fehler) = self.schreibe(&antwort).await {
                    tracing::error!(
                        plugin = "stdout",
                        "Antwort konnte nicht gesendet werden: {fehler}"
                    );
                }
            }
            (Some(kennung), None) => {
                let antwort = match (nachricht.result, nachricht.error) {
                    (_, Some(fehler)) => Err(fehler),
                    (Some(wert), None) => Ok(wert),
                    (None, None) => Err(RpcError {
                        code: RPC_INVALID_PARAMS,
                        message: "Antwort ohne Ergebnis und ohne Fehler".to_string(),
                        data: None,
                    }),
                };
                let nummer = kennung.as_u64().unwrap_or_default();
                let wartekanal = self.offen.lock().await.remove(&nummer);
                if let Some(wartekanal) = wartekanal {
                    let _ = wartekanal.send(antwort);
                }
            }
            (None, Some(methode)) => {
                tracing::info!(plugin = %methode, "Hinweis des Plugins");
            }
            (None, None) => {}
        }
    }

    /// Schreibt einen Rahmen an das Plugin.
    pub async fn schreibe(&self, nachricht: &Message) -> AppResult<()> {
        let mut stdin = self.stdin.lock().await;
        write_message(&mut *stdin, nachricht)
            .await
            .map_err(|fehler| AppError::Internal(format!("Schreiben fehlgeschlagen: {fehler}")))
    }

    /// Sendet eine Anfrage und wartet auf die Antwort.
    ///
    /// Nach Ablauf der Frist wird die Anfrage zurückgezogen. Das Plugin darf
    /// dann noch antworten. Die Antwort wird verworfen.
    pub async fn ruf(&self, methode: &str, params: Value, frist: Duration) -> AppResult<Value> {
        let kennung = self.naechste_kennung.fetch_add(1, Ordering::SeqCst);
        let (sender, empfaenger) = oneshot::channel();
        self.offen.lock().await.insert(kennung, sender);
        let anfrage = Message::request(Value::from(kennung), methode, params);
        if let Err(fehler) = self.schreibe(&anfrage).await {
            self.offen.lock().await.remove(&kennung);
            return Err(fehler);
        }

        let antwort = match tokio::time::timeout(frist, empfaenger).await {
            Ok(Ok(antwort)) => antwort,
            Ok(Err(_)) => {
                self.offen.lock().await.remove(&kennung);
                return Err(AppError::Internal(format!(
                    "Plugin hat die Antwort auf {methode} nie geschickt"
                )));
            }
            Err(_) => {
                self.offen.lock().await.remove(&kennung);
                return Err(AppError::Internal(format!(
                    "Zeitüberschreitung nach {frist:?} bei {methode}"
                )));
            }
        };

        match antwort {
            Ok(wert) => Ok(wert),
            Err(fehler) => Err(AppError::Internal(format!(
                "Plugin meldete Fehler {}: {}",
                fehler.code, fehler.message
            ))),
        }
    }

    /// Sendet einen Hinweis ohne Antwort.
    pub async fn hinweis(&self, methode: &str, params: Value) -> AppResult<()> {
        let nachricht = Message::notification(methode, params);
        self.schreibe(&nachricht).await
    }

    /// Beendet den Kindprozess.
    pub async fn beende(&self) {
        let mut kind = self.kind.lock().await;
        let _ = kind.start_kill();
        let _ = kind.wait().await;
    }

    /// Meldet, ob der Kindprozess noch läuft.
    pub async fn lebt(&self) -> bool {
        self.kind.lock().await.try_wait().ok().flatten().is_none()
    }

    /// Wartet, bis der Kindprozess endet.
    ///
    /// Der Rückgabewert sagt, ob der Prozess innerhalb der Frist endete.
    pub async fn warte_aus(&self, frist: Duration) -> bool {
        let ende = tokio::time::Instant::now() + frist;
        loop {
            if let Ok(Some(_)) = self.kind.lock().await.try_wait() {
                return true;
            }
            if tokio::time::Instant::now() >= ende {
                return false;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }
}

/// Legt das Kind in eine eigene Prozessgruppe.
///
/// Das Kind ruft `setsid` vor dem Start der Programmroutine auf. Damit ist es
/// Leiter einer eigenen Sitzung und von der Prozessgruppe des Hosts getrennt.
/// Das Beenden des Kindes erreicht so keine Prozesse des Hosts.
#[cfg(unix)]
fn eigene_prozessgruppe(befehl: &mut Command) {
    use std::os::unix::process::CommandExt;

    // `setsid` stammt aus libc. Die Crate libc steht nicht als Abhängigkeit
    // zur Verfügung, deshalb bindet der Host die Funktion selbst ein.
    extern "C" {
        fn setsid() -> i32;
    }

    // Der Aufruf von `pre_exec` ist unsicher, weil zwischen fork und exec
    // fremder Code laufen könnte. Der einzelne Aufruf von `setsid` ist
    // sicher, weil er keine-allokierenden Aufruf ist.
    unsafe {
        befehl.as_std_mut().pre_exec(|| {
            let ergebnis = setsid();
            if ergebnis == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

/// Legt das Kind auf Systemen ohne Unix-Sitzungen in eine Prozessgruppe.
#[cfg(not(unix))]
fn eigene_prozessgruppe(_befehl: &mut Command) {}

/// Wandelt einen Fehler des Plugins in einen JSON-RPC-Fehler.
pub fn als_rpc_fehler(fehler: AppError) -> RpcError {
    let code = match fehler {
        AppError::Validation(_) => RPC_INVALID_PARAMS,
        AppError::TaskNotFound(_) => RPC_METHOD_NOT_FOUND,
        _ => RPC_PLUGIN_ERROR,
    };
    RpcError {
        code,
        message: fehler.to_string(),
        data: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    struct Aufzeichner {
        anfragen: Mutex<Vec<String>>,
    }

    #[async_trait]
    impl HostRequestHandler for Aufzeichner {
        async fn handle(&self, methode: &str, _params: Value) -> Result<Value, RpcError> {
            self.anfragen.lock().await.push(methode.to_string());
            Ok(json!({"ok": true}))
        }
    }

    #[tokio::test]
    async fn kindprozess_erbt_keine_geheimnisse() {
        let mut umgebung = BTreeMap::new();
        umgebung.insert("PATH".to_string(), "/usr/bin".to_string());
        umgebung.insert("LANG".to_string(), "C.UTF-8".to_string());
        umgebung.insert("HOME".to_string(), "/tmp".to_string());

        let prozess = PluginProcess::start(
            Path::new("/bin/sh"),
            Path::new("/tmp"),
            &umgebung,
            Arc::new(Aufzeichner {
                anfragen: Mutex::new(Vec::new()),
            }),
        )
        .await;
        assert!(prozess.is_ok(), "Kindprozess startete nicht");
    }

    #[tokio::test]
    async fn kindprozess_sieht_nur_die_positivliste() {
        let mut umgebung = BTreeMap::new();
        umgebung.insert("PATH".to_string(), "/usr/bin:/bin".to_string());
        umgebung.insert("LANG".to_string(), "C.UTF-8".to_string());
        umgebung.insert("HOME".to_string(), "/tmp".to_string());

        let prozess = PluginProcess::start(
            Path::new("/bin/sh"),
            Path::new("/tmp"),
            &umgebung,
            Arc::new(Aufzeichner {
                anfragen: Mutex::new(Vec::new()),
            }),
        )
        .await
        .unwrap();

        let antwort = prozess
            .ruf("pruefen", json!({}), Duration::from_secs(5))
            .await;
        assert!(antwort.is_err(), "das Plugin kennt die Methode nicht");
    }
}
