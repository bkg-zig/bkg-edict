//! Start von edict als stdio-Plugin.
//!
//! Der Host startet das Plugin als eigenen Prozess und spricht mit ihm über
//! stdin und stdout. Auf stdout gehören ausschließlich Protokollrahmen. Die
//! Protokolleinträge gehen nach stderr.

use std::sync::Arc;

use edict_plugin_edict::{serve, EdictPlugin};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Protokollrahmen gehen nach stdout. Die Protokolleinträge gehören nach
    // stderr, damit sie den Rahmen nicht stören.
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    let plugin = Arc::new(EdictPlugin::new());
    serve(tokio::io::stdin(), tokio::io::stdout(), plugin).await;
    Ok(())
}
