//! Host für Hotplug-Plugins.
//!
//! Der Host startet Plugins als eigene Prozesse, spricht mit ihnen über
//! JSON-RPC 2.0 auf stdio und beendet sie wieder.
//!
//! Dieser Platzhalter wird vom Plugin-Subagenten ersetzt.

pub const PROTOCOL_VERSION: u32 = edict_plugin_protocol::PROTOCOL_VERSION;
