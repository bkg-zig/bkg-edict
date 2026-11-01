//! Definition der Befehle und Argumente der Kommandozeile.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

/// Kommandozeile von Edict.
#[derive(Debug, Parser)]
#[command(
    name = "edict",
    version,
    about = "Kommandozeile für Edict",
    long_about = "Die Kommandozeile spricht ausschließlich mit der HTTP-Schnittstelle des Edict-Servers.",
    disable_help_subcommand = true
)]
pub struct Cli {
    /// Adresse des Servers. Sie überschreibt Umgebungsvariablen und Konfigurationsdatei.
    #[arg(long, global = true, value_name = "URL")]
    pub url: Option<String>,

    #[command(subcommand)]
    pub command: Command,
}

/// Oberbefehle der Kommandozeile.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Legt die Konfiguration an und prüft die Erreichbarkeit des Servers.
    Init(InitArgs),
    /// Lädt den Live-Status und speichert ihn lokal als JSON.
    Sync(SyncArgs),
    /// Prüft Server, Datenbank und Gateway.
    Doctor(DoctorArgs),
    /// Aufgaben anzeigen und anlegen.
    Tasks(TasksArgs),
    /// Einzelne Aufgabe bearbeiten.
    Task(TaskArgs),
    /// Status der Agenten anzeigen.
    Agents(AgentsArgs),
    /// Morgenbrief anzeigen.
    Brief(BriefArgs),
    /// Kennzahlen anzeigen.
    Stats(StatsArgs),
    /// Remote-Skills anzeigen.
    Skills(SkillsArgs),
    /// Wire-Vertrag der Serverantworten anzeigen.
    Protocol,
}

/// Argumente von `edict init`.
#[derive(Debug, Args)]
pub struct InitArgs {
    /// Adresse des Servers, die in die Konfigurationsdatei geschrieben wird.
    #[arg(long, value_name = "URL")]
    pub url: Option<String>,

    /// Vorhandene Konfiguration überschreiben.
    #[arg(long)]
    pub force: bool,
}

/// Argumente von `edict sync`.
#[derive(Debug, Args)]
pub struct SyncArgs {
    /// Zieldatei für den Live-Status.
    #[arg(long, value_name = "PFAD")]
    pub output: Option<PathBuf>,
}

/// Argumente von `edict doctor`.
#[derive(Debug, Args)]
pub struct DoctorArgs {
    /// Ergebnis als JSON ausgeben.
    #[arg(long)]
    pub json: bool,
}

/// Argumente von `edict tasks`.
#[derive(Debug, Args)]
pub struct TasksArgs {
    #[command(subcommand)]
    pub command: TasksCommand,
}

/// Unterbefehle von `edict tasks`.
#[derive(Debug, Subcommand)]
pub enum TasksCommand {
    /// Aufgaben auflisten.
    List(ListArgs),
    /// Eine Aufgabe ausführlich anzeigen.
    Show(ShowArgs),
    /// Eine Aufgabe anlegen.
    Create(CreateArgs),
    /// Eine Aufgabe archivieren.
    Archive(ArchiveArgs),
    /// Eine Aufgabe aus dem Archiv holen.
    Unarchive(ArchiveArgs),
}

/// Argumente von `edict tasks list`.
#[derive(Debug, Args)]
pub struct ListArgs {
    /// Zustand der Aufgaben. Erlaubt sind die kanonischen Namen und die deutschen Anzeigenamen.
    #[arg(long, value_name = "ZUSTAND")]
    pub state: Option<String>,

    /// Organisation der Aufgaben.
    #[arg(long, value_name = "ORG")]
    pub org: Option<String>,

    /// Archivierte Aufgaben anzeigen. Ohne diese Angabe werden aktive Aufgaben gezeigt.
    #[arg(long)]
    pub archived: bool,

    /// Ergebnis als JSON ausgeben.
    #[arg(long)]
    pub json: bool,
}

/// Argumente von `edict tasks show`.
#[derive(Debug, Args)]
pub struct ShowArgs {
    /// Kennung der Aufgabe. Fachliche Kennung, technische Kennung oder Spur dürfen angegeben werden.
    #[arg(value_name = "ID")]
    pub id: String,

    /// Ergebnis als JSON ausgeben.
    #[arg(long)]
    pub json: bool,
}

/// Argumente von `edict tasks create`.
#[derive(Debug, Args)]
pub struct CreateArgs {
    /// Titel der Aufgabe.
    #[arg(long, value_name = "TITEL")]
    pub title: String,

    /// Beschreibung der Aufgabe.
    #[arg(long, value_name = "TEXT")]
    pub description: Option<String>,

    /// Priorität der Aufgabe. Erlaubt sind low, normal, high und urgent.
    #[arg(long, value_name = "PRIORITÄT")]
    pub priority: Option<String>,

    /// Agent oder Amt, das die Aufgabe bearbeitet.
    #[arg(long, value_name = "NAME")]
    pub assignee: Option<String>,

    /// Antwort des Servers als JSON ausgeben.
    #[arg(long)]
    pub json: bool,
}

/// Argumente von `edict tasks archive` und `edict tasks unarchive`.
#[derive(Debug, Args)]
pub struct ArchiveArgs {
    /// Kennung der Aufgabe.
    #[arg(value_name = "ID")]
    pub id: String,

    /// Antwort des Servers als JSON ausgeben.
    #[arg(long)]
    pub json: bool,
}

/// Argumente von `edict task`.
#[derive(Debug, Args)]
pub struct TaskArgs {
    #[command(subcommand)]
    pub command: TaskCommand,
}

/// Unterbefehle von `edict task`.
#[derive(Debug, Subcommand)]
pub enum TaskCommand {
    /// Zustand einer Aufgabe wechseln.
    Advance(AdvanceArgs),
    /// Aktion auf eine Aufgabe anwenden.
    Action(ActionArgs),
    /// Teilaufgaben einer Aufgabe setzen.
    Todos(TodosArgs),
    /// Fortschrittsmeldung an eine Aufgabe hängen.
    Progress(ProgressArgs),
    /// Prüfung bestätigen oder zurückweisen.
    Review(ReviewArgs),
}

/// Argumente von `edict task advance`.
#[derive(Debug, Args)]
pub struct AdvanceArgs {
    /// Kennung der Aufgabe.
    #[arg(value_name = "ID")]
    pub id: String,

    /// Zielzustand.
    #[arg(long, value_name = "ZUSTAND")]
    pub to: String,

    /// Agent, der den Wechsel ausführt.
    #[arg(long, value_name = "AGENT")]
    pub agent: String,

    /// Begründung des Wechsels.
    #[arg(long, value_name = "TEXT")]
    pub reason: Option<String>,

    /// Antwort des Servers als JSON ausgeben.
    #[arg(long)]
    pub json: bool,
}

/// Argumente von `edict task action`.
#[derive(Debug, Args)]
pub struct ActionArgs {
    /// Kennung der Aufgabe.
    #[arg(value_name = "ID")]
    pub id: String,

    /// Aktion, zum Beispiel stop, cancel oder resume.
    #[arg(long, value_name = "AKTION")]
    pub action: String,

    /// Zusätzliche Felder als JSON-Objekt.
    #[arg(long, value_name = "JSON")]
    pub payload: Option<String>,

    /// Antwort des Servers als JSON ausgeben.
    #[arg(long)]
    pub json: bool,
}

/// Argumente von `edict task todos`.
#[derive(Debug, Args)]
pub struct TodosArgs {
    /// Kennung der Aufgabe.
    #[arg(value_name = "ID")]
    pub id: String,

    /// Teilaufgaben als JSON-Array.
    #[arg(long, value_name = "JSON")]
    pub from: String,

    /// Antwort des Servers als JSON ausgeben.
    #[arg(long)]
    pub json: bool,
}

/// Argumente von `edict task progress`.
#[derive(Debug, Args)]
pub struct ProgressArgs {
    /// Kennung der Aufgabe.
    #[arg(value_name = "ID")]
    pub id: String,

    /// Agent, der den Fortschritt meldet.
    #[arg(long, value_name = "AGENT")]
    pub agent: String,

    /// Text der Fortschrittsmeldung.
    #[arg(long, value_name = "TEXT")]
    pub content: String,

    /// Antwort des Servers als JSON ausgeben.
    #[arg(long)]
    pub json: bool,
}

/// Argumente von `edict task review`.
#[derive(Debug, Args)]
pub struct ReviewArgs {
    /// Kennung der Aufgabe.
    #[arg(value_name = "ID")]
    pub id: String,

    /// Entscheidung der Prüfung. Erlaubt sind approve und reject.
    #[arg(long, value_name = "ENTSCHEIDUNG")]
    pub action: String,

    /// Begründung der Entscheidung.
    #[arg(long, value_name = "TEXT")]
    pub comment: Option<String>,

    /// Antwort des Servers als JSON ausgeben.
    #[arg(long)]
    pub json: bool,
}

/// Argumente von `edict agents`.
#[derive(Debug, Args)]
pub struct AgentsArgs {
    #[command(subcommand)]
    pub command: AgentsCommand,
}

/// Unterbefehle von `edict agents`.
#[derive(Debug, Subcommand)]
pub enum AgentsCommand {
    /// Status aller Agenten anzeigen.
    Status(StatusArgs),
    /// Aktivität eines Agenten anzeigen.
    Activity(ActivityArgs),
}

/// Argumente von `edict agents status`.
#[derive(Debug, Args)]
pub struct StatusArgs {
    /// Ergebnis als JSON ausgeben.
    #[arg(long)]
    pub json: bool,
}

/// Argumente von `edict agents activity`.
#[derive(Debug, Args)]
pub struct ActivityArgs {
    /// Kennung des Agenten.
    #[arg(value_name = "AGENT")]
    pub agent_id: String,

    /// Ergebnis als JSON ausgeben.
    #[arg(long)]
    pub json: bool,
}

/// Argumente von `edict brief`.
#[derive(Debug, Args)]
pub struct BriefArgs {
    /// Datum des Morgenbriefs im Format JJJJ-MM-TT.
    #[arg(long, value_name = "DATUM")]
    pub date: Option<String>,

    /// Ergebnis als JSON ausgeben.
    #[arg(long)]
    pub json: bool,
}

/// Argumente von `edict stats`.
#[derive(Debug, Args)]
pub struct StatsArgs {
    /// Ergebnis als JSON ausgeben.
    #[arg(long)]
    pub json: bool,
}

/// Argumente von `edict skills`.
#[derive(Debug, Args)]
pub struct SkillsArgs {
    #[command(subcommand)]
    pub command: SkillsCommand,
}

/// Unterbefehle von `edict skills`.
#[derive(Debug, Subcommand)]
pub enum SkillsCommand {
    /// Remote-Skills anzeigen.
    List(SkillsListArgs),
}

/// Argumente von `edict skills list`.
#[derive(Debug, Args)]
pub struct SkillsListArgs {
    /// Ergebnis als JSON ausgeben.
    #[arg(long)]
    pub json: bool,
}
