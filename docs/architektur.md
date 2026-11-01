# Architektur

Edict 2.0 trennt den fachlichen Kern von Datenbank, Netzwerk und
Oberfläche. Die Trennung ist der Grund, warum derselbe Fachlogik im Server,
im Worker und in der Kommandozeile anwendbar ist.

## Crates

`edict-core` enthält Aufgabenmodell, Zustandsmaschine, Anzeigetexte,
Konfiguration und die Servicefunktionen für Zustandswechsel, Fortschritt,
Teilaufgaben und Archivierung. Das Crate kennt weder SQL noch HTTP und
verwendet keine Datenbanktreiber.

`edict-db` enthält die PostgreSQL-Anbindung über SQLx, die Migrationen, den
Redis-Bus für Streams und Pub/Sub sowie die Outbox. Der Redis-Bus schreibt
Stream-Eintrag und Pub/Sub-Nachricht in einem Lua-Skript, damit beide Wege
denselben Ereignisinhalt tragen.

`edict-plugin-protocol` trägt den JSON-RPC-Rahmen für die Plugin-Kommunikation.
Der Rahmen nutzt einen `Content-Length`-Kopf, damit Zeilenumbrüche in der
Nutzlast eindeutig bleiben. Das Crate enthält außerdem die Protokollversion
und die Fehlercodes.

`edict-server` bringt HTTP und WebSocket mit Axum, liefert die Oberfläche
statisch aus und betreibt den Plugin-Host. `edict-worker` liest die Outbox,
ruft Agenten auf und führt den Scheduler aus. `edict-cli` erzeugt das Binary
`edict` und schreibt ausschließlich über die HTTP-Schnittstelle.

`edict-server` und `edict-worker` enthalten im Repository nur einen
Platzhalter. Die beschriebene Aufteilung ist das Ziel des Ports, nicht der
gebaute Stand. `edict-cli` ist übersetzbar.

Die Plugins unter `crates/plugins/` sind eigene Prozesse. Sie teilen keine
Tabelle mit dem Kern und erhalten weder das Admin-Token noch die
Datenbankverbindung.

## Kennungen einer Aufgabe

Eine Aufgabe führt drei Kennungen nebeneinander. `task_id` ist die technische
UUID und dient als Primärschlüssel in PostgreSQL. `id` ist die fachliche
Legacy-Kennung im Format `JJC-YYYYMMDD-NNN` und bleibt für Menschen
lesbar. `trace_id` verbindet die Ereigniskette eines Vorgangs. Die drei
Werte werden nicht zusammengeführt, weil Altbestand und neue Technik
unterschiedliche Schlüssel brauchen.

## Zustandsmaschine

Die Zustände stehen in PascalCase: `Taizi`, `Zhongshu`, `Menxia`, `Assigned`,
`Next`, `Doing`, `Review`, `Done`, `Blocked`, `Cancelled`, `Pending`. Der
Wert `Pending` ist der Startzustand für Aufgaben, die noch nicht sortiert
wurden.

| Von | Nach |
| --- | --- |
| `Taizi` | `Zhongshu`, `Cancelled` |
| `Zhongshu` | `Menxia`, `Cancelled`, `Blocked` |
| `Menxia` | `Assigned`, `Zhongshu`, `Cancelled` |
| `Assigned` | `Doing`, `Next`, `Cancelled`, `Blocked` |
| `Next` | `Doing`, `Cancelled` |
| `Doing` | `Review`, `Done`, `Blocked`, `Cancelled` |
| `Review` | `Done`, `Doing`, `Cancelled` |
| `Blocked` | `Taizi`, `Zhongshu`, `Menxia`, `Assigned`, `Doing` |

`Done` und `Cancelled` sind Endzustände. Ein Wechsel aus `Blocked` darf in
einen beliebigen aktiven Zustand führen, damit eine festgefahrene Aufgabe
wieder aufgenommen werden kann. Die Funktion `can_transition` in
`edict-core` entscheidet jeden Übergang. Fehler werden mit deutscher
Meldung abgewiesen.

## Zuordnung zu Agenten

Die Zuordnung folgt dem Zustand und der Organisation. Für `Taizi`,
`Zhongshu`, `Menxia`, `Assigned` und `Review` ist jeweils ein Agent
zuständig. Die sechs Ministerien werden über die Organisation zugeordnet,
wobei `户部` dem Finanzagenten und `工部` dem Bauagenten gehören.

## Ereignisfluss

Die Themen des Buses folgen dem Muster `task.<ereignis>`, dazu kommen
`agent.thoughts`, `agent.todo.update` und `agent.heartbeat`. Der Worker
schreibt Ereignisse in die Outbox. Das Relay nimmt die unpublished Zeilen
in einer Transaktion mit `FOR UPDATE OF o SKIP LOCKED` und schreibt sie in
den Redis-Stream `edict:stream:<thema>`. Der Stream wird bei 10000
Einträgen gekürzt. Für die Oberfläche veröffentlicht dasselbe Lua-Skript
die Nachricht auf dem Kanal `edict:pubsub:<thema>`.

Die Outbox ist damit die einzige Stelle, an der ein Ereignis entsteht. Eine
Transaktion, die eine Aufgabe ändert, endet nicht ohne unausgeliefertes
Ereignis. Fehlgeschlagene Zustellversuche zählt die Spalte `attempts`, der
letzte Fehlertext steht in `last_error`.

## Aufgabenbewegung und Fortschritt

Der Server schreibt eine Aufgabe nicht direkt in die Datenbank. Er lädt sie,
prüft den Zustandswechsel, ergänzt das Zustandsprotokoll und speichert das
Ergebnis mit einer optimistischen Sperre. Jede Speicherung erhöht das Feld
`version`. Das Zustandsprotokoll wächst je Übergang um einen Eintrag mit
Zeitstempel, Sender, Empfänger und Bemerkung.

Der Herzschalt einer Aufgabe entsteht aus dem Alter der letzten Änderung.
Unter 180 Sekunden gilt die Aufgabe als aktiv, unter 600 Sekunden als
möglicherweise festgefahren, danach als festgefahren. Der Schwellenwert steht
in `STALL_THRESHOLD_SEC`.

## Plugin-Host

Ein Plugin ist ein langlebiger Prozess, den der Host zur Laufzeit startet,
verwaltet und wieder beendet. Die Kommunikation läuft über JSON-RPC 2.0 auf
stdio. Der Host übergibt nur eine Positivliste aus Konfiguration an den
Kindprozess, damit dieser weder das Admin-Token noch die Zugangsdaten der
Datenbank erbt.

Der Host liest je Plugin eine Konsumentengruppe `plugin:<name>` aus dem
Redis-Stream, quittiert eine angenommene Nachricht mit `XACK` und holt
verwaiste Nachrichten mit `XAUTOCLAIM` zurück. Der Aufbau steht in
`docs/plugins.md` und in der Debatte unter `docs/debate/`.

## Dateibasiertes Gedächtnis

Agentenkonfiguration, Skills, Modellwechsel, Morgenbrief und Kennzahlen
liegen als JSON und Text im Verzeichnis `data`. Jede Datei wird unter einer
Sperre atomar geschrieben. Das Verzeichnis hält dieselbe Rolle wie im
Altbestand und bleibt damit für Werkzeuge lesbar, die keinen HTTP-Zugang
haben.
