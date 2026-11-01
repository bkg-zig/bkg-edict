//! Zustände eines Plugins im Host.

use edict_plugin_protocol::PluginState;
use serde::{Deserialize, Serialize};

/// Zustand eines Plugins im Host.
///
/// Die Zustände beschreiben den Lebenszyklus eines Kindprozesses. Sie
/// decken sich nicht mit [`PluginState`] aus dem Protokoll. Das Protokoll
/// kennt nur die Arbeitsfähigkeit eines bereits gestarteten Plugins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginStatus {
    /// Das Plugin ist nicht geladen. Der Host kennt es nur aus dem Manifest.
    NotLoaded,
    /// Der Prozess startet gerade und der Handshake läuft.
    Starting,
    /// Das Plugin arbeitet normal.
    Ready,
    /// Das Plugin arbeitet eingeschränkt, etwa bei voller Ereigniswarteschlange.
    Degraded,
    /// Das Plugin ist nicht arbeitsfähig.
    Failed,
    /// Der Prozess wurde beendet.
    Stopped,
}

impl PluginStatus {
    /// Deutscher Anzeigename des Zustands.
    pub fn label(self) -> &'static str {
        match self {
            PluginStatus::NotLoaded => "nicht geladen",
            PluginStatus::Starting => "startend",
            PluginStatus::Ready => "bereit",
            PluginStatus::Degraded => "eingeschränkt",
            PluginStatus::Failed => "fehlgeschlagen",
            PluginStatus::Stopped => "beendet",
        }
    }

    /// Zustand nach der Protokollvorschrift.
    pub fn protocol_state(self) -> PluginState {
        match self {
            PluginStatus::Ready => PluginState::Ready,
            PluginStatus::Degraded | PluginStatus::Starting => PluginState::Degraded,
            PluginStatus::Failed | PluginStatus::Stopped | PluginStatus::NotLoaded => {
                PluginState::Failed
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zustaende_haben_deutsche_anzeigenamen() {
        assert_eq!(PluginStatus::NotLoaded.label(), "nicht geladen");
        assert_eq!(PluginStatus::Starting.label(), "startend");
        assert_eq!(PluginStatus::Ready.label(), "bereit");
        assert_eq!(PluginStatus::Degraded.label(), "eingeschränkt");
        assert_eq!(PluginStatus::Failed.label(), "fehlgeschlagen");
        assert_eq!(PluginStatus::Stopped.label(), "beendet");
    }

    #[test]
    fn startendes_plugin_gilt_als_eingeschraenkt() {
        assert_eq!(
            PluginStatus::Starting.protocol_state(),
            PluginState::Degraded
        );
        assert_eq!(PluginStatus::Ready.protocol_state(), PluginState::Ready);
        assert_eq!(PluginStatus::Failed.protocol_state(), PluginState::Failed);
    }
}
