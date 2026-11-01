//! Umsetzung der Befehle der Kommandozeile.

pub mod agents;
pub mod data;
pub mod ops;
pub mod tasks;

use anyhow::Result;

use crate::api::ApiClient;
use crate::cli::{Cli, Command};

/// Führt den gewählten Befehl aus.
pub async fn run(cli: Cli) -> Result<()> {
    match &cli.command {
        Command::Init(args) => ops::init(cli.url.as_deref(), args).await,
        Command::Sync(args) => {
            let client = client(&cli)?;
            ops::sync(&client, args).await
        }
        Command::Doctor(args) => {
            let client = client(&cli)?;
            ops::doctor(&client, args).await
        }
        Command::Tasks(args) => {
            let client = client(&cli)?;
            tasks::run_tasks(&client, args).await
        }
        Command::Task(args) => {
            let client = client(&cli)?;
            tasks::run_task(&client, args).await
        }
        Command::Agents(args) => {
            let client = client(&cli)?;
            agents::run(&client, args).await
        }
        Command::Brief(args) => {
            let client = client(&cli)?;
            ops::brief(&client, args).await
        }
        Command::Stats(args) => {
            let client = client(&cli)?;
            ops::stats(&client, args).await
        }
        Command::Skills(args) => {
            let client = client(&cli)?;
            ops::skills(&client, args).await
        }
        Command::Protocol => ops::protocol(),
    }
}

/// Erzeugt den Client aus Option, Umgebung und Konfigurationsdatei.
pub fn client(cli: &Cli) -> Result<ApiClient> {
    let (url, _source) = crate::config::resolve_base_url(cli.url.as_deref())?;
    let token = std::env::var("EDICT_ADMIN_TOKEN")
        .ok()
        .filter(|value| !value.trim().is_empty());
    ApiClient::new(&url, token)
}
