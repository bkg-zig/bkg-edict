//! Host für Hotplug-Plugins.
//!
//! Der Host startet Plugins als eigene Prozesse, spricht mit ihnen über
//! JSON-RPC 2.0 auf stdio und beendet sie wieder.
//!
//! # Aufbau
//!
//! - [`manifest`] liest und prüft das Manifest eines Plugins.
//! - [`env`] baut die Umgebung des Kindprozesses. Sie enthält nur
//!   `PATH`, `LANG` und `HOME`.
//! - [`transport`] startet den Kindprozess und führt die Verbindung.
//! - [`plugin`] hält Manifest, Prozess, Zustand und Ereigniswarteschlange
//!   eines Plugins.
//! - [`registry`] hält die geladenen Plugins und löst Namen zur
//!   Anfragezeit auf.
//! - [`http`] leitet HTTP-Anfragen an Plugins weiter.
//! - [`services`] beschreibt die Schnittstellen des Hosts zur Umgebung.
//!
//! # Sicherheit
//!
//! Der Kindprozess erbt keine Umgebung des Hosts. Das Admin-Token und die
//! Zugangsdaten der Datenbank erreichen das Plugin nicht. Plugins binden
//! ihre HTTP-Schnittstelle an `127.0.0.1`.
//!
//! # Beispiel
//!
//! ```no_run
//! use std::sync::Arc;
//! use edict_plugin_host::{HostConfig, PluginRegistry};
//! use edict_plugin_host::services::{NoopEventSink, NoopServices};
//!
//! # async fn beispiel() -> Result<(), Box<dyn std::error::Error>> {
//! let registry = PluginRegistry::new(
//!     Arc::new(NoopServices),
//!     Arc::new(NoopEventSink),
//!     Arc::new(HostConfig::default()),
//! );
//! let info = registry.load_dir(std::path::Path::new("/pfad/zu/gitclaw")).await?;
//! println!("{} läuft", info.name);
//! registry.shutdown().await;
//! # Ok(())
//! # }
//! ```

pub mod env;
pub mod http;
pub mod manifest;
pub mod plugin;
pub mod registry;
pub mod services;
pub mod state;
pub mod transport;

pub use manifest::Manifest;
pub use plugin::{HostBridge, HostConfig, PluginHandle, PluginInfo};
pub use registry::PluginRegistry;
pub use services::{
    ChannelEventSource, EventSink, EventSource, HostServices, NoopEventSink, NoopServices,
};
pub use state::PluginStatus;

/// Version des Protokolls, die der Host erwartet.
pub const PROTOCOL_VERSION: u32 = edict_plugin_protocol::PROTOCOL_VERSION;
