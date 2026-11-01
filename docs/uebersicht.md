# Übersicht

Edict 2.0 ist eine Aufgabenverwaltung nach dem Vorbild der drei Kanzleien
und der sechs Ministerien. Aufgaben durchlaufen eine feste Zustandsmaschine,
Ereignisse laufen über einen Ereignisbus, und mehrere Agenten arbeiten an
demselben Vorgang. Die Oberfläche und die Werkzeuge sind deutschsprachig.

## Bestandteile

| Crate | Aufgabe | Stand im Repository |
| --- | --- | --- |
| `edict-core` | Fachlicher Kern ohne Datenbank und Netzwerk | implementiert |
| `edict-db` | PostgreSQL, Redis Streams, Pub/Sub, Outbox | implementiert |
| `edict-plugin-protocol` | JSON-RPC-Rahmen für Plugins | implementiert |
| `edict-server` | HTTP, WebSocket, Plugin-Host | Platzhalter |
| `edict-worker` | Orchestrator, Dispatcher, Scheduler | Platzhalter |
| `edict-cli` | Binary `edict` | implementiert |
| `crates/plugins/gitclaw` | Prüfung von Merge-Anfragen | Platzhalter |
| `crates/plugins/edict` | Mehragenten-Workflows | Platzhalter |

`crates/edict-web` ist im Repository noch nicht angelegt. Die Oberfläche
entsteht als Dioxus-Anwendung für das Ziel `wasm32-unknown-unknown` und
liegt bewusst außerhalb des Workspace.

## Datenfluss

1. Der Server nimmt Aufgabenbefehle über HTTP an und prüft jeden
   Zustandswechsel gegen die Zustandsmaschine in `edict-core`.
2. Die Datenbank schreibt Aufgabe und Ereignis in einer Transaktion. Die
   Outbox in derselben Transaktion verhindert, dass ein Ereignis verloren
   geht.
3. Der Worker liest die Outbox, schreibt die Ereignisse in Redis und ruft
   für Aufgaben in der Arbeit das Gateway ZeptoClaw auf.
4. Die Oberfläche hält eine WebSocket-Verbindung und empfängt die Ereignisse
   über Redis Pub/Sub.

## Erste Schritte

```bash
cp .env.example .env
docker compose build
docker compose up -d postgres redis
```

Der Bau des Images scheitert, solange `edict-server` und `edict-worker` keine
lauffähigen Binaries liefern. Der Stand ist im Abschnitt „Offene Punkte“ in
`docs/betrieb.md` beschrieben.

## Dokumentation

- `docs/architektur.md` beschreibt Crates, Zustandsmaschine und Ereignisfluss.
- `docs/api.md` beschreibt HTTP-Routen und WebSocket-Nachrichten.
- `docs/betrieb.md` beschreibt Dienste, Konfiguration, Sicherung und Beobachtung.
- `docs/cli.md` beschreibt die Befehle des Binaries `edict`.
- `docs/plugins.md` beschreibt Manifest und Protokoll der Plugins.
- `docs/uebersetzung.md` beschreibt die Übersetzung der Altbestände.
- `docs/zeptoclaw.md` beschreibt die Anbindung des Gateways.
- `docs/migration-von-python.md` beschreibt den Weg vom Altbestand zum Port.
- `docs/debate/` hält die sichtbaren Fachdebatten mit Abstimmung fest.
