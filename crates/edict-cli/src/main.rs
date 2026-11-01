//! Kommandozeile von Edict.
//!
//! Der Zugriff auf Aufgaben läuft ausschließlich über die HTTP-Schnittstelle
//! des Servers. Die CLI hält keine Datenbankverbindung und liest keine
//! Dateien des Datenbestands direkt.

mod api;
mod cli;
mod commands;
mod config;
mod output;
mod state;

use std::process::ExitCode;

use clap::Parser;

use crate::cli::Cli;

fn main() -> ExitCode {
    dotenvy::dotenv().ok();
    let cli = Cli::parse();
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("Fehler: Die Laufzeitumgebung konnte nicht gestartet werden: {error}");
            return ExitCode::FAILURE;
        }
    };
    match runtime.block_on(commands::run(cli)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Fehler: {error:#}");
            ExitCode::FAILURE
        }
    }
}
