# Edict 2.0

Edict ist eine Aufgabenverwaltung nach dem Vorbild der三省六部. Aufgaben durchlaufen
eine feste Zustandsmaschine, Ereignisse laufen über einen Ereignisbus, und mehrere
Agenten arbeiten an denselben Vorgängen. Die Oberfläche ist deutschsprachig und als
Dioxus-Anwendung gebaut.

Diese README ist der verbindliche Feature-Record des Projekts. Sie beschreibt, was
gebaut wird, welche Verträge gelten und wie der Betrieb aussieht.

## Zielbild

Der Vorgänger in `edict-2.0` bestand aus zwei Python-Produkten und einem
React-Frontend. Beide Python-Stacks sind durch Rust ersetzt, das Frontend durch Dioxus.

| Bereich | Vorgänger | Ziel |
| --- | --- | --- |
| Aufgaben und Ereignisse | FastAPI, SQLAlchemy, Alembic | `edict-server` mit Axum, `edict-db` mit SQLx |
| Arbeitsteilung | Redis Streams, Redis Pub/Sub | `edict-db` mit Redis Streams und Pub/Sub |
| Orchestrierung | `orchestrator_worker.py`, `dispatch_worker.py` | `edict-worker` |
| Dateibasiertes Gedächtnis | Python-Skripte unter `scripts/` | `edict-worker` mit denselben Unterbefehlen |
| Oberfläche | React, Vite | `edict-web` mit Dioxus, ausgeliefert als WASM |
| Bedienung | Python-Skripte | `edict-cli` mit dem Binary `edict` |
| Erweiterungen | keine | Hotplug-Plugins über JSON-RPC |

Im produktiven Betrieb läuft kein Python-Code mehr.

## Funktionsumfang

### Aufgabenverwaltung

- Aufgaben mit Legacy-Kennung (`JJC-YYYYMMDD-NNN`) und technischer UUID. Beide
  Kennungen werden gleichzeitig geführt und nicht zusammengeführt.
- Zustände in PascalCase: `Taizi`, `Zhongshu`, `Menxia`, `Assigned`, `Next`, `Doing`,
  `Review`, `Done`, `Blocked`, `Cancelled`, `Pending`.
- Zustandswechsel laufen über eine geprüfte Zustandsmaschine. Unzulässige Übergänge
  werden mit einer deutschen Fehlermeldung abgewiesen.
- Jede Aufgabe führt ein Zustandsprotokoll, ein Fortschrittsprotokoll, Teilaufgaben,
  Tags, Vorlagenparameter und Scheduler-Metadaten.
- Der Herzschlag einer Aufgabe wird aus dem letzten Änderungszeitpunkt berechnet und
  meldet aktiv, warnend oder festgefahren.

### Ereignisbus

- Themen wie `task.created`, `task.dispatch`, `task.review.request`, `task.stalled`
  und `agent.heartbeat`.
- Redis Streams tragen die Arbeitsteilung, Redis Pub/Sub die sofortige
  Benachrichtigung der Oberfläche.
- Eine Outbox in PostgreSQL stellt sicher, dass eine Transaktion mit Aufgaben- und
  Ereignisänderung nie ohne ausgeliefertes Ereignis endet.

### Agenten und Dateien

- Agentenzuordnung aus Zustand und Organisation.
- Agentenkonfiguration, Skills, Modellwechsel, Morgenbrief und Kennzahlen bleiben
  dateibasiert im Verzeichnis `data/`. Das entspricht dem bisherigen Vertrag.
- Atomisches Schreiben mit Dateisperre für alle JSON- und Textdateien.

### Oberfläche

- Dioxus-Anwendung, gebaut für `wasm32-unknown-unknown`.
- Der Axum-Server liefert das WASM-Bundle statisch aus. Es gibt keinen zweiten
  Webserver und keinen Fullstack-Server von Dioxus.
- Deutsche Beschriftungen, Leerzustände, Ladeanzeigen und Fehlermeldungen.
- Live-Aktualisierung über WebSocket mit Protokoll `ping`, `pong`, `subscribe`,
  `subscribed` und `event`.

### Kommandozeile

Das Binary `edict` schreibt ausschließlich über die HTTP-API. Ein JSON-Dateifallback
existiert nicht.

- `kanban create`, `state`, `flow`, `done`, `block`, `todo`, `progress`
- `migrate import-json`
- `skill list`, `list-remote`, `add-remote`, `update-remote`, `remove-remote`,
  `check-updates`
- `model set`, `log`, `last-result`
- `morning-brief show`, `refresh`, `config`
- `task list`, `show`, `stats`, `live-status`, `archive`, `review`

### Hotplug-Plugins

Der Host kann Erweiterungen zur Laufzeit annehmen, auswerfen und austauschen, ohne
dass der Server neu startet. Ein Plugin ist ein eigener Prozess, der über JSON-RPC
auf stdio mit dem Host spricht.

- Ein Plugin liefert ein Manifest mit Name, Version, Fähigkeiten und Initialisierung.
- Fähigkeiten benennen, welche Aufgabenereignisse das Plugin verarbeitet und welche
  HTTP-Routen es anbietet.
- Der Host startet, stoppt und ersetzt Plugins, ohne den Kern neu zu bauen.
- Plugins teilen keine Datenbanktabelle mit dem Kern. Eigener Namensraum pro Plugin.
- Ein Plugin erhält keine Tokens der Oberfläche und keine unbeschränkten
  Shell-Befehle.

Die ersten beiden Plugins sind aus Fremdprojekten abgeleitet, aber vollständig neu
in Rust geschrieben:

- **gitclaw** prüft Merge-Anfragen. Features: Webhook-Eingang von GitHub, GitLab und
  Bitbucket, Sperrregeln für Entwürfe, Bot-Autoren, doppelte Prüfsummen und
  `[skip review]`, Erzeugung strukturierter Befunde, Zusammenfassung und Inline-Kommentare,
  Unterstützung mehrerer KI-Anbieter, statische Analyse, Konfiguration je Repository.
- **edict** unterstützt Multi-Agent-Workflows. Features: Projektinitialisierung,
  Synchronisation eingebetteter Workflow-Dokumente, `doctor`-Prüfungen, Agenten-Schleifen
  mit Journal, Protokollbefehle für Zustandsübergänge, Review-Gate, Koordination über
  Beads und Ereignisprotokolle, Hook-System.

Der Quellcode dieser beiden Projekte dient als Referenz für die Features. Die
Implementierung in diesem Repository ist eigenständig.

## Verzeichnisstruktur

```
bkg-edict/
├── crates/
│   ├── edict-core/     Fachlicher Kern ohne Datenbank- und Netzwerktreiber
│   ├── edict-db/       SQLx-PostgreSQL, Redis Streams, Pub/Sub, Outbox
│   ├── edict-server/   Axum-HTTP, WebSocket, statische Auslieferung, Plugin-Host
│   ├── edict-worker/   Orchestrator, Dispatcher, Scheduler, Synchronisation
│   ├── edict-cli/      Binary edict
│   └── edict-web/      Dioxus-Oberfläche, eigenes Cargo.toml, wasm32
├── docs/               Architektur, API, Betrieb, CLI, Übersetzung, Debatten
├── data/               Dateibasiertes Gedächtnis
├── scripts/            Betriebshilfen in Bash
├── .brain/             Aufgaben, Entscheidungen, Blockaden, Memory
├── Dockerfile
└── docker-compose.yml
```

`edict-web` liegt bewusst außerhalb des Workspace. Das WASM-Ziel baut nicht mit
denselben Flags wie die Server-Binaries.

## Voraussetzungen

- Rust 1.83 oder neuer
- PostgreSQL 14 oder neuer
- Redis 7 oder neuer
- Node 20 oder neuer und `dx` 0.7.10 für den Oberflächenbau

## Schnellstart

```bash
cp .env.example .env
docker compose up --build
```

Die Oberfläche ist danach unter `http://localhost:8000` erreichbar, der API-Zugang
unter `http://localhost:8000/healthz`.

## Lokaler Build

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --workspace
cargo build --locked --release
```

Oberfläche:

```bash
cd crates/edict-web
dx build --platform web
```

Das Ergebnis liegt in `crates/edict-web/dist`.

## Konfiguration

| Variable | Vorgabe | Bedeutung |
| --- | --- | --- |
| `DATABASE_URL` | aus `POSTGRES_*` | PostgreSQL-Verbindung |
| `REDIS_URL` | `redis://localhost:6379/0` | Redis-Verbindung |
| `BACKEND_HOST` | `0.0.0.0` | Bindeadresse des Servers |
| `BACKEND_PORT` | `8000` | Port des Servers |
| `EDICT_ALLOWED_ORIGINS` | `http://localhost:7891` | CORS-Allowlist, Kommagetrennt |
| `EDICT_ADMIN_TOKEN` | ohne | Token für Admin-Routen |
| `LEGACY_DATA_DIR` | `../data` | Verzeichnis des dateibasierten Gedächtnisses |
| `LEGACY_TASKS_FILE` | `../data/tasks_source.json` | Datei der Aufgaben im Legacy-Format |
| `OPENCLAW_BIN` | `openclaw` | Programm für den Aufruf der Agenten |
| `OPENCLAW_PROJECT_DIR` | ohne | Verzeichnis der OpenClaw-Sessions |
| `EDICT_DISPATCH_TIMEOUT_SEC` | `300` | Zeitlimit für einen Dispatch |
| `EDICT_MAX_CONCURRENT_DISPATCH` | `3` | Zahl gleichzeitiger Dispatches |

`EDICT_ALLOWED_ORIGINS` ist eine Allowlist. Der Wert `*` wird abgewiesen, weil der
Server Anmeldedaten erlaubt.

## Datenmodell

- `tasks` mit `task_id` als UUID, `legacy_id` als eindeutiger `JJC-`-Kennung und
  Zustand als Text mit Prüfbedingung.
- `events` mit Ereigniskennung, Spur, Zeitstempel, Thema, Typ, Erzeuger, Nutzlast.
- `outbox` für Ereignisse, die nach einer Transaktion veröffentlicht werden.
- `todos`, `thoughts` und `meta` je Aufgabe.

Die führende Datenquelle für Aufgaben und Ereignisse ist PostgreSQL.

## Migration vom Python-Bestand

```bash
edict migrate import-json --file data/tasks_source.json --dry-run
edict migrate import-json --file data/tasks_source.json
```

Der Import ordnet Legacy-Kennung und Zustand zu, schreibt je Aufgabe ein
`task.created`-Ereignis und lässt den Scheduler-Zustand unberührt. Vor dem ersten
Produktivimport ist ein Backup der PostgreSQL-Instanz zu erstellen.

## Sicherheit

- CORS nur über die Allowlist aus `EDICT_ALLOWED_ORIGINS`.
- Admin-Routen verlangen `EDICT_ADMIN_TOKEN` über den Kopf `X-Admin-Token`.
- Passwörter und Tokens erscheinen nicht in Protokollen und Fehlermeldungen.
- Redis läuft ohne Eviction, weil die Streams kein Cache sind.
- Plugins laufen als getrennte Prozesse und kommen nicht an die Datenbankverbindung
  des Kerns.

## Projektgeschichtliche Quellen

Die Verträge stammen aus dem Vorgängerprojekt `edict-2.0`, das als Referenz
dient:

- `dashboard/server.py` für die Legacy-HTTP-Routen und das Dateiformat
- `edict/backend/app/api/` für die UUID-Routen, das WebSocket-Protokoll und die Admin-Routen
- `edict/backend/app/services/event_bus.py` für den Redis-Streams-Vertrag
- `edict/backend/app/services/task_service.py` für die Geschäftsregeln
- `edict/backend/app/workers/` für die Semantik von Orchestrator und Dispatcher
- `edict/frontend/src/api.ts` für den HTTP-Vertrag der Oberfläche
- `edict/migration/versions/001_initial.py` für das Ausgangsschema

Der Bestand bleibt unverändert. Er wird nicht mehr produktiv ausgeführt.

## Dokumentation

- `docs/architektur.md` Crates, Datenfluss, Zustandsmaschine, Ereignisfluss, Plugin-Host
- `docs/api.md` alle HTTP-Routen und WebSocket-Nachrichten
- `docs/betrieb.md` Dienste, Umgebungsvariablen, Migrationen, Sicherung, Beobachtung
- `docs/cli.md` alle Befehle des Binaries `edict`
- `docs/plugins.md` Plugin-Manifest, JSON-RPC-Vertrag, Fähigkeiten, Beispiele
- `docs/uebersetzung.md` übersetzte Dokumente und Oberflächentexte
- `docs/debate/` die sichtbaren Fachdebatten mit Abstimmung

## Lizenz

MIT
# bkg-edict
