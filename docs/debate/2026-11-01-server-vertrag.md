# Debatte: Serververtrag des Rust-Ports

Datum: 01.11.2026
Bezug: DEC-SERVER-001, RUST-PORT-SERVER-001
Fachleute: Serverbau, Vertragsprüfung, Produkt und Betrieb

## Fragestellung

Der Altbestand hat zwei Server mit zwei verschiedenen Verträgen. Der Rust-Port
braucht einen Server, der beide bedient. Welche Datenquelle ist die fachliche
Wahrheit, welche Formate antworten die Routen, wie schützt der Server die
Adminrouten, wo endet die Kompatibilität zum Altbestand und wo beginnt eine
bewusste Abweichung?

## Belege aus dem Bestand

### Zwei Verträge im Altbestand

Das Dashboard ist ein eigener HTTP-Handler
(`/home/openchamber/dump/coding/edict-2.0/dashboard/server.py:2122` für die
Leseanfragen und `:2201` für die Schreibanfragen). Das Backend ist eine
FastAPI-Anwendung mit eigenen Routern
(`/home/openchamber/dump/coding/edict-2.0/edict/backend/app/main.py:67-72`).

Beide benutzen dieselben Daten, aber unterschiedliche Formen:

| Route | Altbestand | Form |
| --- | --- | --- |
| `GET /api/live-status` | `dashboard/server.py:2131-2132` liest `live_status.json` | `tasks` ist eine Liste, dazu `officials`, `history`, `metrics`, `syncStatus`, `health` |
| `GET /api/tasks/live-status` | `edict/backend/app/api/tasks.py:110-121` ruft `TaskService.get_live_status` | `tasks` und `completed_tasks` sind_maps über die Aufgabenkennung, dazu `last_updated` |

Die Karte des FastAPI-Vorgängers entsteht in
`edict/backend/app/services/task_service.py:260-275`: laufende und beendete
Aufgaben stehen getrennt in zwei Wörterbüchern.

Der Rust-Kern bildet die Dashboardform bereits ab. `LiveStatus` in
`crates/edict-core/src/model.rs:398-418` trägt `tasks` als `Vec<Task>` und
benennt die übrigen Felder genauso wie die Datei des Altbestands. Die
Aufgabenliste entsteht über `TaskRepository::live_status`
(`crates/edict-core/src/task_store.rs:721-747`). Die beendeten Aufgaben
bestimmt `edict_core::state_machine::is_terminal`
(`crates/edict-core/src/state_machine.rs:120-122`).

### Der Altbestand schützt die Adminrouten nicht

Die Adminrouten des Altbestands prüfen kein Token. `deep_health` in
`edict/backend/app/api/admin.py:16-40` und `pending_events` ab `:43` hängen
nur an der Datenbankverbindung und am Ereignisbus. Eine Suche über
`edict/backend/app/` findet weder `X-Admin-Token` noch einen Vergleich mit
einem Token.

Auch das Dashboard kennt keine Prüfung. Jeder Request auf `/api/agent-wake`
oder `/api/set-model` wirkt direkt auf den Bestand.

Der Kern liest das Token bereits aus der Umgebung
(`crates/edict-core/src/config.rs:86`). Die CLI sendet es in beiden Formen,
als `Authorization: Bearer` und als `X-Admin-Token`
(`crates/edict-cli/src/api.rs:14-15` und `:117-122`).

### Der Altbestand erlaubt alle Herkünfte

`edict/backend/app/main.py:59-60` setzt die CORS-Middleware auf
`allow_origins=["*"]`. Der Kern liest die Allowlist aus
`EDICT_ALLOWED_ORIGINS` (`crates/edict-core/src/config.rs:53-62`).

### Fehlerformate im Altbestand

Das Dashboard antwortet mit `{'ok': True, ...}` oder
`{'ok': False, 'error': ...}`. Für fehlende Eingaben antwortet es mit
Status 400 und derselben Hülle, etwa
`dashboard/server.py:2395-2397` für `create-task` und `:2410-2412` für
`review-action`.

Der Kern kennt `AppError` mit `code()` und `status()`
(`crates/edict-core/src/error.rs:54-78`). `AppError::Conflict` bedeutet dort
Status 409.

### WebSocket

Der Altbestand beantwortet `ping` mit `pong` und `subscribe` mit
`subscribed` samt der Themenliste
(`edict/backend/app/api/websocket.py:87-93`). Die Ereignisse kommen als
`{"type": "event", "topic": ..., "data": ...}`
(`edict/backend/app/api/websocket.py:71-73`). Ein Abonnement schränkt die
Zustellung nicht ein, der Client bekommt alle Ereignisse aus dem Muster
`edict:pubsub:*` (`:39`).

### Pluginrouten und Rumpfgrenzen

Der Plugin-Host leitet Anfragen an Plugins weiter
(`crates/edict-plugin-host/src/http.rs`). Die Rumpfgrenze liegt dort bei
8 MiB (`crates/edict-plugin-host/src/http.rs:25`). Die Weiterleitung ist
ausdrücklich registriert und fängt keine fremden Pfade ab.

### Kennungen

Das Dashboard arbeitet mit der Kennung `JJC-YYYYMMDD-NNN`. Der Kern bildet
sie über `edict_core::legacy_id::next_legacy_id`
(`crates/edict-core/src/legacy_id.rs:18`). Der CLI sucht eine Aufgabe über
die UUID und über die Altbestandskennung
(`crates/edict-core/src/repository.rs:27-93`, `TaskFilter`).

## Positionen

### Serverbau: zustimmend

Der Server braucht einen Router, der beide Altbestandsverträge bedient und
darin die Datenbank als Quelle führt. Die Konfigurationsdateien des
Dashboards bleiben Dateien, weil sie im Altbestand Dateien sind und die
Oberfläche sie unverändert liest. Eine Rückführung auf die Datenbank würde
die Formate ändern und den Import aufwändiger machen.

Der Live-Status braucht beide Formen, weil zwei Kunden sie lesen. Die
Dashboardform steht bereits im Kern bereit. Die Karte des FastAPI-Vorgängers
baut der Server aus derselben Liste, indem er die beendeten Aufgaben
abtrennt.

Für die Adminrouten genügt das Token aus `EDICT_ADMIN_TOKEN`. Der Kern liest
es, die CLI sendet es in beiden Formen. Fehlt der Token, antwortet der
Server mit 401, ist der konfigurierte Wert leer, antwortet er mit 403.

### Vertragsprüfung: überarbeitet

Die Position des Serverbaus ist im Kern richtig, aber sie lässt drei Punkte
offen, die den Vertrag in der Praxis brechen.

Erstens liegt die Fehlerform des Altbestands nicht fest. Das Dashboard
antwortet bei fachlichen Fehlern mit HTTP 200 und `ok: false`, bei fehlenden
Eingaben mit HTTP 400. Der Kern kennt 409 für Konflikte. Ohne Festlegung
schreibt jedes Team eine eigene Hülle.

Zweitens fehlt eine Grenze für den Rumpf der JSON-Routen. Der Plugin-Host
begrenzt auf 8 MiB, der Kern hat keine Grenze. Eine unbegrenzte Anfrage
belegt den Speicher des Servers.

Drittens fehlt die Abgrenzung der Pluginroute. Ein Aufrufer darf nicht
beliebige Pfade an ein Plugin durchreichen können.

### Produkt und Betrieb: überarbeitet

Die CORS-Allowlist des Kerns wird zur Bedingung. Ein Aufbau mit `*` ist im
Altbestand erlaubt, im Port aber nicht, weil der Server Anmeldedaten
verwaltet. Der Start muss abbrechen, wenn die Allowlist `*` enthält, sonst
glaubt der Betreiber, die Oberfläche sei abgesichert.

Die Oberfläche läuft über dieselbe Herkunft wie die API. Der Vorgabewert der
Allowlist muss deshalb zum Port der Oberfläche passen.

Der Betrieb braucht einen Health-Check, der Datenbank und Ereignisbus
getrennt nennt. Der Altbestand kennt ihn als `/healthz`
(`dashboard/server.py:2126-2130`). Der Server braucht ihn für Docker.

## Risiken und Gegenmaßnahmen

**Zwei Fehlerwelten.** Fachliche Fehler der Dashboardrouten bleiben bei HTTP
200 mit `ok: false`, weil die Oberfläche den Statuscode nicht prüft, sondern
das Feld. Die Kennzahlen und Ereignisrouten des FastAPI-Vorgängers
antworten mit den Statuscodes des Kerns. Gegenmaßnahme: Die Fehlerhülle
`{"ok": false, "error": "..."}` für alle Routen, die Zugriffsfehler mit dem
Statuscode des Kerns.

**Der Altbestand kennt kein Admin-Token.** Wer die Routen ungeschützt
übernimmt, verliert eine Schutzgrenze, die der Altbestand nie hatte. Der
Port darf sie nicht verschärfen, ohne die Oberfläche anzupassen. Gegenmaßnahme:
Das Token wird Pflicht für die Adminrouten, die CLI sendet es bereits in
beiden Formen.

**Unbegrenzte Rümpfe.** Gegenmaßnahme: 1 MiB für die JSON-Routen, 8 MiB für
die Pluginrouten, Überschreitung ergibt 413.

**Fremde Pfade im Pluginproxy.** Ein Pfad mit `..` oder ein Präfix-Trick in
der Adresse würde den Pfad verlassen. Gegenmaßnahme: Die Adresse wird mit
einem URL-Parser geprüft, der Host muss exakt `127.0.0.1`, `localhost` oder
`[::1]` sein, und `X-Admin-Token`, `Authorization` und `Cookie` werden vor
der Weiterleitung entfernt.

**Die Karte und die Liste sprechen von derselben Sache.** Beide Formen
entstehen aus derselben Aufgabenliste. Ein Wechsel der Sicht ändert nichts
am Bestand. Gegenmaßnahme: Der Server baut beide Formen aus einer Abfrage.

**Zwei Erzeuger für dieselbe Legacy-Kennung.** Die Nummerierung
`JJC-YYYYMMDD-NNN` zählt ab 1 je Tag. Zwei gleichzeitige Anfragen können
dieselbe Nummer vergeben. Gegenmaßnahme: Die Vergabe läuft über
`next_legacy_id` mit der Liste der vorhandenen Kennungen und einer
eindeutigen Prüfung.

## Empfehlung

1. PostgreSQL ist die fachliche Wahrheit für Aufgaben, Zustände, Scheduler,
   Ereignisse und Statistik. Konfigurationsdateien bleiben Dateien und
   werden über `edict_core::filestore` gelesen und geschrieben.
2. `GET /api/live-status` liefert die Dashboardform mit `tasks` als Liste.
   `GET /api/tasks/live-status` liefert die Karte des FastAPI-Vorgängers mit
   `tasks` und `completed_tasks`.
3. Das Admin-Token kommt aus `EDICT_ADMIN_TOKEN`. Primär trägt es
   `X-Admin-Token`, zusätzlich wird `Authorization: Bearer` angenommen. Ein
   leerer konfigurierter Token sperrt die Adminrouten mit 403. Ein fehlender
   oder falscher Token ergibt 401.
4. Die CORS-Allowlist kommt aus `EDICT_ALLOWED_ORIGINS`. Der Wert `*` bricht
   den Start ab. Erlaubt sind GET, POST, PUT und OPTIONS. Der Vorgabewert ist
   `http://localhost:8080`.
5. Die Fehlerhülle lautet `{"ok": false, "error": "deutsche Meldung"}`.
   Fachliche Fehler der Dashboardrouten bleiben bei HTTP 200.
6. Die Rumpfgrenze liegt bei 1 MiB für JSON-Routen und 8 MiB für
   Pluginrouten. Überschreitung ergibt 413.
7. `POST /api/create-task` nimmt `org` und `assignee` sowie `description`.
   `POST /api/advance-state` nimmt ein optionales `to`. Die Fortschrittsroute
   `/api/task-progress` kommt hinzu, `/api/task-action` nimmt zusätzlich
   `action: "progress"`.
8. Der WebSocket quittiert ein Abonnement ohne Filterung. Die Routen heißen
   `/ws` und `/ws/task/{taskId}`.
9. Die Pluginrouten sind ausdrücklich registriert, `/api/plugins/{name}` und
   `/api/plugins/{name}/{*rest}`.
10. Der SPA-Fallback gilt nur außerhalb von `/api/`.
11. Die Altbestandskennung entsteht über `next_legacy_id`. Das Feld `id` der
    gesendeten Aufgabe ist diese Kennung.

## Votum

| Fachmann | Votum | Kern |
| --- | --- | --- |
| Serverbau | zustimmend | Datenbank als Wahrheit, Dateien für Konfiguration, beide Live-Status-Formen, Tokenpflicht |
| Vertragsprüfung | überarbeitet | Fehlerform, Rumpfgrenze und Abgrenzung der Pluginroute fehlen |
| Produkt und Betrieb | überarbeitet | CORS-Allowlist ohne `*`, Vorgabewert passend zur Oberfläche, Health-Check |

Eine zustimmende Mehrheit von zwei Fachleuten liegt nicht vor. Die Debatte
wird nach der Regel des Projekts an den Arbiter eskaliert.

## Entscheidung des Arbiter

Der Arbiter übernimmt die Punkte 1 bis 11 der Empfehlung als verbindliche
Vorgabe für `RUST-PORT-SERVER-001`. Die drei offenen Punkte aus der
Vertragsprüfung und der Produktposition sind damit geschlossen.

Verbindlich ist außerdem:

- Der Server bricht den Start ab, wenn die Allowlist `*` enthält. Eine leere
  Allowlist ist erlaubt und lässt nur Aufrufe derselben Herkunft zu.
- Die Pluginroute prüft die Zieladresse mit einem URL-Parser. Der Host muss
  exakt `127.0.0.1`, `localhost` oder `[::1]` sein. `X-Admin-Token`,
  `Authorization` und `Cookie` gehen nicht an das Plugin.
- Die Kennungen der Aufgaben werden vor dem Anlegen auf Eindeutigkeit
  geprüft.
- Jede Route antwortet in deutscher Sprache. Fehlermeldungen nennen die
  Ursache, keine englischen Bezeichner der Implementierung.

Umsetzung: in `RUST-PORT-SERVER-001`. Der Server lag zum Stand dieser
Debatte als Platzhalter vor (`crates/edict-server/src/main.rs`).
