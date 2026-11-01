//! Registry der geladenen Plugins.
//!
//! Die Registry hält die Griffe der Plugins und kennt ihre Zustände. Sie
//! löst den Namen eines Plugins erst zur Anfragezeit auf. Dadurch sieht der
//! Server immer den aktuellen Stand, auch wenn ein Plugin neu geladen oder
//! entladen wurde.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use edict_core::{AppError, AppResult};
use tokio::sync::RwLock;

use crate::manifest::Manifest;
use crate::plugin::{HostBridge, HostConfig, PluginHandle, PluginInfo};
use crate::services::{EventSink, HostServices};
use crate::state::PluginStatus;

/// Registry der geladenen Plugins.
#[derive(Clone)]
pub struct PluginRegistry {
    plugins: Arc<RwLock<HashMap<String, Arc<PluginHandle>>>>,
    config: Arc<HostConfig>,
    bridge: Arc<HostBridge>,
}

impl PluginRegistry {
    /// Legt eine Registry an.
    ///
    /// `services` beantwortet Aufrufe der Plugins an den Host. `sink` nimmt
    /// Domainereignisse an, die ein Plugin meldet.
    pub fn new(
        services: Arc<dyn HostServices>,
        sink: Arc<dyn EventSink>,
        config: Arc<HostConfig>,
    ) -> Self {
        Self {
            plugins: Arc::new(RwLock::new(HashMap::new())),
            config,
            bridge: Arc::new(HostBridge { services, sink }),
        }
    }

    /// Liest das Manifest eines Verzeichnisses und lädt das Plugin.
    pub async fn load_dir(&self, dir: &Path) -> AppResult<PluginInfo> {
        let manifest = Manifest::load(dir)?;
        self.load(manifest, dir.to_path_buf()).await
    }

    /// Lädt ein Plugin aus seinem Manifest.
    ///
    /// Ein bereits geladenes Plugin gleichen Namens wird zuerst beendet.
    pub async fn load(&self, manifest: Manifest, dir: PathBuf) -> AppResult<PluginInfo> {
        if self.plugins.read().await.contains_key(&manifest.name) {
            self.unload(&manifest.name).await?;
        }

        let handle = PluginHandle::new(manifest, dir, self.config.clone());
        handle.start(self.bridge.clone()).await?;
        let info = handle.plugin_info().await;
        self.plugins.write().await.insert(info.name.clone(), handle);
        Ok(info)
    }

    /// Entlädt ein Plugin und beendet seinen Prozess.
    pub async fn unload(&self, name: &str) -> AppResult<()> {
        let handle = self.plugins.read().await.get(name).cloned();
        let Some(handle) = handle else {
            return Err(AppError::Conflict(format!(
                "Plugin {name} ist nicht geladen"
            )));
        };
        handle.stop().await?;
        self.plugins.write().await.remove(name);
        Ok(())
    }

    /// Startet ein Plugin neu.
    ///
    /// Schlägt der Start fehl, wartet der Host nach dem Fehlversuch. Die
    /// Wartezeit wächst mit jedem Versuch. Nach der letzten Wartezeit endet
    /// der Wiederanlauf.
    pub async fn restart(&self, name: &str) -> AppResult<PluginInfo> {
        let handle = self.plugins.read().await.get(name).cloned();
        let Some(handle) = handle else {
            return Err(AppError::Conflict(format!(
                "Plugin {name} ist nicht geladen"
            )));
        };
        handle.stop().await?;

        let mut versuch = 0;
        let mut wartezeit = self.config.restart_backoff_base;
        loop {
            versuch += 1;
            match handle.start(self.bridge.clone()).await {
                Ok(()) => return Ok(handle.plugin_info().await),
                Err(fehler) => {
                    if versuch >= self.config.restart_max_attempts {
                        handle.melde_fehler(fehler.to_string()).await;
                        return Err(fehler);
                    }
                    tracing::warn!(
                        plugin = %name,
                        versuch,
                        "Start fehlgeschlagen, neuer Versuch folgt: {fehler}"
                    );
                    tokio::time::sleep(wartezeit).await;
                    wartezeit *= self.config.restart_backoff_factor.max(1);
                }
            }
        }
    }

    /// Findet ein Plugin nach Namen.
    ///
    /// Die Auflösung erfolgt zur Anfragezeit.
    pub async fn lookup(&self, name: &str) -> Option<Arc<PluginHandle>> {
        self.plugins.read().await.get(name).cloned()
    }

    /// Findet ein Plugin, das eine HTTP-Adresse meldet.
    pub async fn lookup_http(&self, name: &str) -> AppResult<Arc<PluginHandle>> {
        let handle = self
            .lookup(name)
            .await
            .ok_or_else(|| AppError::Conflict(format!("Plugin {name} ist nicht geladen")))?;
        let info = handle.info().await;
        if info.http_base_url.is_none() {
            return Err(AppError::Conflict(format!(
                "Plugin {name} bietet keine HTTP-Schnittstelle an"
            )));
        }
        Ok(handle)
    }

    /// Listet alle geladenen Plugins auf.
    pub async fn list(&self) -> Vec<PluginInfo> {
        let handles: Vec<Arc<PluginHandle>> = self.plugins.read().await.values().cloned().collect();
        let mut infos = Vec::with_capacity(handles.len());
        for handle in handles {
            infos.push(handle.plugin_info().await);
        }
        infos.sort_by(|links, rechts| links.name.cmp(&rechts.name));
        infos
    }

    /// Zustand eines Plugins.
    pub async fn state(&self, name: &str) -> Option<PluginStatus> {
        let handle = self.lookup(name).await?;
        Some(handle.zustand().await)
    }

    /// Ruft eine Fähigkeit eines Plugins auf.
    pub async fn invoke(
        &self,
        name: &str,
        capability: &str,
        params: serde_json::Value,
    ) -> AppResult<serde_json::Value> {
        let handle = self
            .lookup(name)
            .await
            .ok_or_else(|| AppError::Conflict(format!("Plugin {name} ist nicht geladen")))?;
        handle.invoke(capability, params).await
    }

    /// Verteilt ein Ereignis an alle abonnierenden Plugins.
    ///
    /// Der Host wartet nicht auf die Zustellung. Eine volle Warteschlange
    /// zählt das Ereignis als verworfen.
    pub async fn dispatch_event(&self, ereignis: &edict_core::Event) {
        let handles: Vec<Arc<PluginHandle>> = self.plugins.read().await.values().cloned().collect();
        for handle in handles {
            handle.queue(ereignis.clone()).await;
        }
    }

    /// Beendet alle Plugins.
    pub async fn shutdown(&self) {
        let handles: Vec<Arc<PluginHandle>> = self.plugins.read().await.values().cloned().collect();
        for handle in handles {
            let _ = handle.stop().await;
        }
        self.plugins.write().await.clear();
    }
}

/// Wartezeit, die der Host nach einem fehlgeschlagenen Start wartet.
///
/// Der Wert wächst mit jedem Versuch. Er ist nach außen sichtbar, damit der
/// Server seine eigene Wiederanlaufregel anpassen kann.
pub fn wartezeit(base: Duration, versuch: u32, faktor: u32) -> Duration {
    base.saturating_mul(faktor.max(1).saturating_pow(versuch.saturating_sub(1)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::{NoopEventSink, NoopServices};

    fn registry() -> PluginRegistry {
        PluginRegistry::new(
            Arc::new(NoopServices),
            Arc::new(NoopEventSink),
            Arc::new(HostConfig::default()),
        )
    }

    #[tokio::test]
    async fn unbekanntes_plugin_wird_gemeldet() {
        let registry = registry();
        assert!(registry.lookup("gibt-es-nicht").await.is_none());
        let fehler = registry.unload("gibt-es-nicht").await.unwrap_err();
        assert!(matches!(fehler, AppError::Conflict(_)));
    }

    #[tokio::test]
    async fn leere_registry_zeigt_keine_plugins() {
        let registry = registry();
        assert!(registry.list().await.is_empty());
    }

    #[test]
    fn wartezeit_waechst_mit_jedem_versuch() {
        let basis = Duration::from_millis(100);
        assert_eq!(wartezeit(basis, 1, 2), Duration::from_millis(100));
        assert_eq!(wartezeit(basis, 2, 2), Duration::from_millis(200));
        assert_eq!(wartezeit(basis, 3, 2), Duration::from_millis(400));
    }
}
