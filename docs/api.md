# HTTP-Schnittstelle und WebSocket

Diese Seite beschreibt den Vertrag, den der Server erfüllen soll. Der
Quellcode von `edict-server` ist im Repository noch ein Platzhalter. Die
Routen sind daher als Ziel vereinbart und nicht als gemessene Laufzeit.

Der API-Client der Oberfläche liegt in `crates/edict-web/src/api.rs`. Er
spricht eine Teilmenge der unten genannten Zustands-, Datei- und
Aufgabenrouten an. Der Client ist damit kein Beleg für einen laufenden
Server.

Der Vertrag stammt aus zwei Vorgängern. Das Dashboard `dashboard/server.py`
lieferte die Datei- und Scheduler-Routen. Das FastAPI-Backend unter
`edict/backend/app/api/` lieferte die Aufgaben-, Ereignis-, Agenten- und
Admin-Routen mit UUID sowie das WebSocket-Protokoll. Feldnamen und
Verhalten des neuen Kernels folgen dem Legacy-Vertrag, damit bestehende
Oberflächen weiterarbeiten.

## Adresse

Im Container hört der Server auf `BACKEND_HOST` und `BACKEND_PORT`. Die
Betriebsdatei setzt `BACKEND_PORT=8080`, Docker veröffentlicht den Port
über `EDICT_HTTP_PORT`. Im Standard ist die Oberfläche unter
`http://localhost:8080` erreichbar.

## Zustands- und Dateirouten

| Methode | Pfad | Zweck |
| --- | --- | --- |
| GET | `/healthz` | Prüft Datenverzeichnis, Lesbarkeit und Schreibrecht der Aufgabendatei |
| GET | `/health` | Alias zu `/healthz` für die Prüfläufe von Docker |
| GET | `/api/live-status` | Liefert den Live-Status des Boards als Liste |
| GET | `/api/agent-config` | Liefert die Agentenkonfiguration |
| GET | `/api/model-change-log` | Liefert das Protokoll der Modellwechsel |
| GET | `/api/last-result` | Liefert das Ergebnis des letzten Modellwechsels |
| GET | `/api/officials-stats` | Liefert die Kennzahlen der Amtsinhaber |
| GET | `/api/morning-brief` | Liefert den Morgenbrief |
| GET | `/api/morning-config` | Liefert die Konfiguration des Morgenbriefs |
| GET | `/api/morning-brief/{datum}` | Liefert den Morgenbrief eines Tages im Format `YYYYMMDD` |
| GET | `/api/remote-skills-list` | Listet die verfügbaren entfernten Skills |
| GET | `/api/skill-content/{agentId}/{skillName}` | Liefert den Inhalt eines Skills |
| GET | `/api/task-activity/{taskId}` | Liefert die Aktivität einer Aufgabe |
| GET | `/api/scheduler-state/{taskId}` | Liefert den Scheduler-Zustand einer Aufgabe |
| GET | `/api/agents-status` | Liefert den Status aller Agenten |
| GET | `/api/agent-activity/{agentId}` | Liefert die Aktivität eines Agenten |

| Methode | Pfad | Zweck |
| --- | --- | --- |
| POST | `/api/morning-config` | Speichert die Konfiguration des Morgenbriefs |
| POST | `/api/morning-brief/refresh` | Startet die Sammlung für den Morgenbrief |
| POST | `/api/scheduler-scan` | Prüft alle Aufgaben auf Stillstand |
| POST | `/api/scheduler-retry` | Startet eine festgefahrene Aufgabe erneut |
| POST | `/api/scheduler-escalate` | Eskaliert eine Aufgabe zu einer höheren Instanz |
| POST | `/api/scheduler-rollback` | Setzt eine Aufgabe auf einen früheren Zustand zurück |
| POST | `/api/repair-flow-order` | Repariert die Reihenfolge eines Zustandsprotokolls |
| POST | `/api/add-skill` | Legt einen Skill für einen Agenten an |
| POST | `/api/add-remote-skill` | Übernimmt einen Skill aus einer entfernten Quelle |
| POST | `/api/remote-skills-list` | Listet die entfernten Skills |
| POST | `/api/update-remote-skill` | Aktualisiert einen entfernten Skill |
| POST | `/api/remove-remote-skill` | Entfernt einen entfernten Skill |
| POST | `/api/task-action` | Führt `stop`, `cancel` oder `resume` aus |
| POST | `/api/archive-task` | Archiviert eine Aufgabe oder alle abgeschlossenen Aufgaben |
| POST | `/api/task-todos` | Schreibt die Teilaufgaben einer Aufgabe |
| POST | `/api/create-task` | Legt eine Aufgabe an |
| POST | `/api/review-action` | Bestätigt oder weist eine Prüfung zurück |
| POST | `/api/advance-state` | Führt einen Zustandswechsel aus |
| POST | `/api/agent-wake` | Weckt einen Agenten auf |
| POST | `/api/set-model` | Stellt einen Modellwechsel für einen Agenten ein |

## Aufgaben-, Ereignis- und Agentenrouten

Diese Routen stammen aus dem FastAPI-Backend. Der Präfix `/api/tasks` gilt
für die ersten beiden Gruppen.

| Methode | Pfad | Zweck |
| --- | --- | --- |
| GET | `/api/tasks` | Listet Aufgaben nach Zustand und Filter |
| GET | `/api/tasks/live-status` | Liefert den Live-Status als Karte |
| GET | `/api/tasks/stats` | Liefert Kennzahlen zu den Aufgaben |
| POST | `/api/tasks` | Legt eine Aufgabe an und antwortet mit Status 201 |
| GET | `/api/tasks/{taskId}` | Liefert eine Aufgabe |
| POST | `/api/tasks/{taskId}/transition` | Wechselt den Zustand einer Aufgabe |
| POST | `/api/tasks/{taskId}/dispatch` | Startet die Ausführung einer Aufgabe |
| POST | `/api/tasks/{taskId}/progress` | Schreibt einen Fortschrittseintrag |
| PUT | `/api/tasks/{taskId}/todos` | Schreibt die Teilaufgaben |
| PUT | `/api/tasks/{taskId}/scheduler` | Schreibt den Scheduler-Zustand |
| GET | `/api/tasks/by-legacy/{legacyId}` | Liefert eine Aufgabe über ihre Legacy-Kennung |
| POST | `/api/tasks/by-legacy/{legacyId}/transition` | Wechselt den Zustand über die Legacy-Kennung |
| POST | `/api/tasks/by-legacy/{legacyId}/progress` | Schreibt Fortschritt über die Legacy-Kennung |
| PUT | `/api/tasks/by-legacy/{legacyId}/todos` | Schreibt Teilaufgaben über die Legacy-Kennung |
| GET | `/api/events` | Listet Ereignisse nach Zeitraum und Thema |
| GET | `/api/events/stream-info` | Liefert den Zustand der Streams |
| GET | `/api/events/topics` | Listet die bekannten Themen |
| GET | `/api/agents` | Listet die Agenten |
| GET | `/api/agents/{agentId}` | Liefert einen Agenten |
| GET | `/api/agents/{agentId}/config` | Liefert die Konfiguration eines Agenten |

Die Kommandozeile sendet einen Fortschrittseintrag an `/api/task-progress`.
Diese Route steht nicht im Altbestand. Antwortet der Server mit 404, wiederholt
das Binary den Aufruf über `/api/task-action`.

## Adminrouten

| Methode | Pfad | Zweck |
| --- | --- | --- |
| GET | `/api/admin/health/deep` | Prüft Datenbank, Redis und Outbox |
| GET | `/api/admin/pending-events` | Zeigt die Ereignisse, die noch nicht ausgeliefert sind |
| POST | `/api/admin/migrate/check` | Prüft, ob eine Migration aussteht |
| GET | `/api/admin/config` | Liefert die Konfiguration ohne Geheimnisse |

Adminrouten verlangen den Wert aus `EDICT_ADMIN_TOKEN`. Der Server nimmt ihn
in zwei Formen an, als Kopfzeile `X-Admin-Token: <wert>` oder als
`Authorization: Bearer <wert>`. Ist die Variable nicht gesetzt oder leer,
bleiben die Routen gesperrt und antworten mit Status 403. Fehlt die
Kopfzeile oder passt der Wert nicht, antwortet der Server mit Status 401.
Ein Token gehört nie in eine URL und nie in ein Protokoll.

## Pluginrouten

| Methode | Pfad | Zweck |
| --- | --- | --- |
| alle | `/api/plugins/{name}` | Leitet die Anfrage an das genannte Plugin weiter |
| alle | `/api/plugins/{name}/{*rest}` | Leitet die Anfrage mit Pfadrest weiter |

Die Routen sind ausdrücklich registriert und fangen keine fremden Pfade ab.
Der Host löst den Namen zur Anfragezeit auf. Die Zieladresse muss `http` auf
`127.0.0.1`, `localhost` oder `[::1]` sein, sonst lehnt der Host die
Weiterleitung ab. Die Kopfzeilen `X-Admin-Token`, `Authorization` und
`Cookie` werden entfernt, bevor die Anfrage das Plugin erreicht. Für diese
Routen gilt eine Rumpfgrenze von 8 MiB.

## WebSocket

Der Server bietet `/ws` für alle Ereignisse und `/ws/task/{taskId}` für die
Ereignisse einer Aufgabe. Die Oberfläche sendet eine Nachricht mit dem Feld
`type`.

| Richtung | `type` | Bedeutung |
| --- | --- | --- |
| Client zu Server | `ping` | Fragt nach dem Zustand der Verbindung |
| Server zu Client | `pong` | Antwort auf `ping` |
| Client zu Server | `subscribe` | Meldet die gewünschten Themen im Feld `topics` |
| Server zu Client | `subscribed` | Bestätigt die abonnierten Themen |
| Server zu Client | `event` | Liefert ein Ereignis mit den Feldern `topic` und `data` |

Der Server liest die Ereignisse aus den Redis-Kanälen `edict:pubsub:<thema>`
und schickt sie ohne Änderung weiter.

## Formate

Ein Aufgabenobjekt trägt die Feldnamen des Altbestands. `id` ist die
Legacy-Kennung, `task_id` die technische UUID und `trace_id` die Spur der
Ereigniskette. Weitere Felder sind `title`, `state`, `org`, `official`,
`now`, `eta`, `block`, `output`, `ac`, `review_round`, `archived`,
`archivedAt`, `flow_log` und `updatedAt`. `flow_log` ist eine Liste von
Einträgen mit den Feldern `at`, `from`, `to` und `remark`.

Die beiden Live-Status-Routen liefern verschiedene Formen, weil zwei
Vorgänger sie getrennt verwendeten.

`GET /api/live-status` liefert die Form des Dashboards. `tasks` ist eine
Liste, daneben stehen `syncStatus` als Objekt und, sofern die Dateien
vorhanden sind, `officials`, `history`, `metrics` und `health`.

`GET /api/tasks/live-status` liefert die Karte des FastAPI-Vorgängers.
`tasks` und `completed_tasks` sind Maps über die Aufgabenkennung, dazu
kommt `last_updated`. Die Trennung entsteht über
`edict_core::state_machine::is_terminal`, also über die Endzustände `Done`
und `Cancelled`.

Ein Ereignis trägt `event_id`, `trace_id`, `timestamp`, `topic`,
`event_type`, `producer`, `payload` und `meta`.

Fehler antworten mit dem Objekt `{"ok": false, "error": "deutsche Meldung"}`.
Ungültiges JSON wird mit Status 400 abgewiesen. Ein zu großer Rumpf wird
mit Status 413 abgewiesen.

## CORS

Der Server liest die erlaubten Herkünfte aus `EDICT_ALLOWED_ORIGINS`. Der
Wert ist eine Allowlist und wird kommagetrennt gelesen. Der Wert `*` ist
unzulässig, weil der Server Anmeldedaten erlaubt.
