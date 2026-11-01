# Migration vom Python-Bestand

Der Vorgänger bestand aus zwei Python-Produkten und einem React-Frontend.
Beide Python-Stacks sind durch Rust ersetzt, das Frontend durch eine
Dioxus-Anwendung. Der Altbestand bleibt unverändert und wird nicht mehr
produktiv ausgeführt. Er dient als Referenz für die Verträge.

## Zuordnung

| Bereich des Vorgängers | Vorgänger | Port |
| --- | --- | --- |
| Aufgaben und Ereignisse | FastAPI, SQLAlchemy, Alembic | `edict-server` mit Axum, `edict-db` mit SQLx |
| Arbeitsteilung | Redis Streams, Redis Pub/Sub | `edict-db` mit Redis Streams und Pub/Sub |
| Orchestrierung | `orchestrator_worker.py`, `dispatch_worker.py` | `edict-worker` |
| Dateibasiertes Gedächtnis | Skripte unter `scripts/` | `edict-worker` mit denselben Unterbefehlen |
| Oberfläche | React, Vite | `edict-web` mit Dioxus, ausgeliefert als WASM |
| Bedienung | Python-Skripte | `edict-cli` mit dem Binary `edict` |
| Erweiterungen | dateibasierte Skills und Hooks | Hotplug-Plugins über JSON-RPC |

## Unterschiede im Datenmodell

Der Altbestand führte eine Aufgabe unter einer Kennung. Der Port führt drei
Kennungen nebeneinander. `task_id` ist die technische UUID und dient als
Primärschlüssel. `legacy_id` ist die fachliche Kennung im Format
`JJC-YYYYMMDD-NNN` und bleibt für Menschen lesbar. `trace_id` verbindet die
Ereigniskette eines Vorgangs. Ein Import führt die Legacy-Kennung nach
`legacy_id` und vergibt eine neue UUID.

Der Live-Status lieferte im Altbestand je Kennung eine Map. Der Port
liefert `tasks` als Liste, weil die Oberfläche eine geordnete Liste
braucht. Clients, die nach Kennung suchen, müssen die Liste jetzt selbst
indexieren.

Die Tabelle `events` im Altbestand war leer, weil alle Ereignisse über
Redis liefen. Der Port legt jedes Ereignis in PostgreSQL ab und führt die
Zustellung über eine Outbox. Damit kann eine Transaktion nicht ohne
ausgeliefertes Ereignis enden.

Redis lief im Altbestand mit `allkeys-lru`. Diese Verdrängung konnte
Ereignisse löschen, während die Arbeitsteilung noch lief. Der Port
verzichtet auf eine Verdrängung und schreibt das AOF-Verfahren, damit die
Streams einen Neustart überstehen.

Der Altbestand erlaubte in CORS jede Herkunft, obwohl er Anmeldedaten
erlaubte. Der Port liest eine Allowlist aus `EDICT_ALLOWED_ORIGINS`.

## Ablauf der Migration

1. PostgreSQL 16 und Redis 7 bereitstellen. Die Compose-Datei des Projekts
   erledigt das mit `docker compose up -d postgres redis`.
2. Den Altbestand sichern:

   ```bash
   docker compose exec postgres pg_dump -U edict -Fc edict > edict.dump
   ```

3. Das Schema anlegen. Die Migrationen unter
   `crates/edict-db/migrations/` legen die Tabellen `tasks`, `todos`,
   `thoughts`, `events` und `outbox` an und legen die Ausführungssperren in
   `dispatch_leases` an. Sie laufen beim Start des Workers.
4. Die Aufgaben importieren. Dieser Schritt ist noch nicht möglich. Es gibt
   weder einen Befehl der Kommandozeile noch eine Route für den Import. Die
   Demodaten in `data/tasks_source.json` liegen bereit und werden nach
   `/app/data/tasks_source.json` eingebunden.
5. Den Import prüfen, sobald er existiert. Je Aufgabe entsteht dann ein
   `task.created`-Ereignis. Der Scheduler-Zustand bleibt unberührt.
6. Die Oberfläche bauen und einbinden. Das Ergebnis aus
   `crates/edict-web/dist` wird nach `/app/web` eingebunden.
7. Den Server und den Worker starten und die Adminrouten prüfen.

## Zuordnung beim Import

| Altbestand | Port |
| --- | --- |
| `id` | `legacy_id` und `id` in der API |
| `state` | `state` mit den kanonischen Werten in PascalCase |
| `flow_log` | `flow_log` als JSONB |
| `review_round` | `review_round` |
| `archived` und `archivedAt` | `archived` und `archived_at` |
| `now`, `eta`, `block`, `output`, `ac` | gleichnamige Spalten |
| keine Angabe | `task_id` als neue UUID, `trace_id` als neue Spur |

Die Werte der Zustände bleiben unverändert. Die sichtbaren Texte werden
beim Import nicht übersetzt. Die Übersetzung geschieht bei der Anzeige.

## Offene Punkte

- Die Skripte des Altbestands unter `scripts/` haben noch keine
  Entsprechung im Worker. Der Morgenbrief, der Wechsel des Modells und die
  Synchronisation der Agentenkonfiguration gehören zu diesen Skripten.
- Der Import über die Kommandozeile ist im Repository noch nicht
  implementiert. Die Route `/api/admin/migrate/check` prüft nur, ob eine
  Migration aussteht.
- Der Altbestand kennt keinen Webhook-Eingang. Der gitclaw-Plugin bringt
  einen eigenen HTTP-Eingang mit, den der Host weiterleitet.
