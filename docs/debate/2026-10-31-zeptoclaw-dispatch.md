# Debatte: ZeptoClaw-Dispatch im Rust-Port

Datum: 31.10.2026
Bezug: DEC-GATEWAY-001, RUST-PORT-WORKER-001
Fachmann: Worker-Dispatch und Gateway-Anbindung

## Fragestellung

Wie ruft der Rust-Worker den Agenten auf, welche Ereignisse entstehen dabei,
wie wird ein Auftrag genau einmal ausgeführt, woran ist die Ausführung gebunden
und welche Fehler werden wiederholt, welche beenden den Auftrag sichtbar?

## Belege aus dem Bestand

### Aufrufsyntax von ZeptoClaw

Die CLI-Definition kennt für den Befehl `agent` genau fünf Schalter
(`/tmp/opencode/refs/zeptoclaw/src/cli/mod.rs:66-82`):

| Schalter | Wirkung laut Quelle |
| --- | --- |
| `-m`, `--message` | Nachricht für den Einmalbetrieb |
| `--template` | eingebautes oder benutzereigenes Template, das unter `~/.zeptoclaw/templates` liegt |
| `--no-stream` | Streaming abschalten, es ist sonst voreingestellt an |
| `--dry-run` | nur anzeigen, welche Werkzeuge laufen würden |
| `--mode` | `observer`, `assistant` oder `autonomous` |

Ein Schalter `--agent` existiert in ZeptoClaw nicht. Die Suche über
`/tmp/opencode/refs/zeptoclaw/src` findet als einzigen Treffer einen Kommentar
zur ACP-Kompatibilität (`src/cli/mod.rs:791`). Die Aufrufform des Legacy
`openclaw agent --agent xxx -m "..."`
(`/home/openchamber/dump/coding/edict-2.0/edict/backend/app/workers/dispatch_worker.py:9`
und `:153-157`) ist damit nicht übertragbar.

Weitere belegte Eigenschaften des Aufrufs:

- Das Arbeitsverzeichnis fällt nur dann auf das aktuelle Verzeichnis zurück,
  wenn die Konfiguration den Vorgabewert `~/.zeptoclaw/workspace` unverändert
  enthält (`src/cli/agent.rs:247-253`). Ein gesetzter eigener Wert schaltet
  den Rückfall ab.
- `ZEPTOCLAW_AGENTS_DEFAULTS_WORKSPACE` überschreibt das Arbeitsverzeichnis
  aus der Umgebung (`src/config/mod.rs:144-146`). Das Konfigurationsverzeichnis
  liegt unter `~/.zeptoclaw` (`src/config/mod.rs:52-56`).
- Die Rolle entsteht über die Systemanweisung. Vorrang hat die Konfiguration,
  dann das Template, dann die aktive Hand (`src/cli/common.rs:290-296`).
  Zusätzlich li ZeptoClaw eine `SOUL.md` aus dem Arbeitsverzeichnis
  (`src/cli/common.rs:275-288`).
- Templates lädt die Registry aus `~/.zeptoclaw/templates`, als `.json` und
  `.toml` (`src/cli/common.rs:83-90`, `src/config/templates.rs:340-371`). Ein
  unbekannter Name bricht mit einer Liste der vorhandenen Namen ab
  (`src/cli/common.rs:105-109`).
- Die Antwort steht auf stdout, Werkzeugmeldungen und die Fußzeile auf stderr
  (`src/cli/ui.rs:415-441` und `:480-494`). Ohne TTY wird der Antworttext
  unverändert geschrieben.
- Erfolg endet mit Exitcode 0, Fehler mit 1
  (`src/main.rs:9-14`, `src/cli/agent.rs:339-342`).
- ZeptoClaw kennt keine `EDICT_`-Variablen. Die Suche über `src` endet ohne
  Treffer. `EDICT_TASK_ID` und `EDICT_TRACE_ID` aus dem Legacy
  (`dispatch_worker.py:160-161`) wirken daher nur als geerbte Umgebung des
  Kindprozesses.
- Die Idempotenzprüfung des Gateways liegt in
  `src/gateway/idempotency.rs:6-51`. Sie ist ein Speicher im Prozess mit
  Ablaufzeit und Größengrenze und dient der Webhook-Entzerrung. Sie taugt
  nicht als dauerhafte Zustellgarantie für Dispatches.

### Ereignisvertrag

Der Rahmen ist in beiden Beständen gleich. `build_event` erzeugt
Ereigniskennung, Zeitstempel und Thema, die Spur kommt aus der Aufgabe
(`/home/openchamber/dump/coding/bkg-edict/crates/edict-core/src/service.rs:223-240`).
Der Legacy-Bus baut dasselbe Muster
(`/home/openchamber/dump/coding/edict-2.0/edict/backend/app/services/event_bus.py:159-167`).

| Ereignis | Thema | Erzeuger | Nutzlast | Legacy | edict-core |
| --- | --- | --- | --- | --- | --- |
| `task.dispatch.request` | `task.dispatch` | `task_service` | `task_id`, `agent`, `message`, `state` | `services/task_service.py:165-188` | `service.rs:171-187` |
| `task.dispatch.request` | `task.dispatch` | `orchestrator` | `task_id`, `agent`, `state`, `message` | `workers/orchestrator_worker.py:131-142` und `:165-169` | fehlt |
| `agent.dispatch.start` | `agent.heartbeat` | `dispatcher` | `task_id`, `agent` | `workers/dispatch_worker.py:104-110` | `service.rs:206-217` |
| `agent.output` | `agent.thoughts` | `agent.<agent>` | `task_id`, `agent`, `output`, `return_code` | `workers/dispatch_worker.py:116-127` | `service.rs:190-203` |

Die Themen und der Strompräfix stehen in
`crates/edict-core/src/model.rs:425-445`. Im Legacy entsprechen sie
`services/event_bus.py:35`, `:43`, `:45` und `:48`.

Der Legacy kennt zwei Erzeuger für `task.dispatch.request`. Der Orchestrator
versendet sie automatisch bei Zustandswechseln, der Task-Dienst auf Anfrage
der Oberfläche. `edict-core` kennt nur `task_service`
(`service.rs:184`). Für den Port fehlt damit die Seite, die aus einem
Zustandswechsel einen Dispatch macht.

### Zustellung in Redis und PostgreSQL

- Der Bus schreibt Strom und Pub/Sub in einem Lua-Skript
  (`crates/edict-db/src/redis_bus.rs:23-35` und `:86-119`). Der Strom ist auf
  10.000 Einträge begrenzt (`:20`).
- Die Konsumentengruppe wird bei Bedarf angelegt (`:122-139`).
- `read_group` liest mit `>` nur neue Einträge (`:142-167`). Offene Einträge
  bleiben im Warteschirm, bis `XAUTOCLAIM` sie übernimmt (`:183-207`).
- Der Outbox-Relay nimmt offene Zeilen mit `FOR UPDATE OF o SKIP LOCKED`,
  veröffentlicht und markiert danach im selben Commit als zugestellt
  (`crates/edict-db/src/outbox.rs:47-103`). Bricht der Commit nach der
  Veröffentlichung ab, wird beim nächsten Durchlauf erneut veröffentlicht.
- Der Worker-Aufruf `request_dispatch` sperrt die Aufgabe mit `FOR UPDATE`
  und legt Ereignis und Outbox in einer Transaktion an
  (`crates/edict-db/src/task_store.rs:754-772` und `:156-194`).
- `EventRepository::append_event` nimmt keine Transaktion
  (`task_store.rs:805-812`). Ereignis und Outbox landen dort in zwei
  getrennten automatischen Commits. Der Worker für die beiden
  Dispatch-Ereignisse braucht diese Methode nicht, er braucht eine
  Transaktion.
- Das Schema kennt keine Sperr- oder Dispatchtabelle. `tasks.version` ist
  eine optimistische Sperre (`crates/edict-db/migrations/0001_init.sql:41`).
  Auf Ereignistypen gibt es keine Eindeutigkeit, nur Suchindizes
  (`0001_init.sql:106-110`).
- Im Rust-Bestand ruft niemand `claim_stale` auf. Die Suche über `crates`
  findet nur die Definition in `redis_bus.rs:183`. Der Legacy-Worker rief
  `claim_stale` nur beim Start auf, mit 60 Sekunden Ruhezeit
  (`dispatch_worker.py:71-78`), bei einem Zeitlimit von 300 Sekunden pro
  Aufruf (`:172`).

### Identitätsbindung

- `task_id` ist die technische UUID, `legacy_id` die fachliche Kennung im
  Format `JJC-YYYYMMDD-NNN`, `trace_id` verbindet die Ereigniskette
  (`0001_init.sql:1-6`). `Task` führt beide Kennungen getrennt
  (`crates/edict-core/src/model.rs:204-212`), die Datenbank liest sie getrennt
  (`task_store.rs:327-329`).
- Der Legacy trägt in der Dispatch-Nutzlast die UUID
  (`services/task_service.py:180`). Die Rollen-ID `JJC-...` steht in den
  Rollendateien, dort als `JJC-xxx`
  (`/home/openchamber/dump/coding/edict-2.0/agents/hubu/SOUL.md:29-43`).
- Die Agentenverzeichnisse des Legacy enthalten elf `SOUL.md` mit den Rollen
  `zaochao`, `shangshu`, `zhongshu`, `menxia`, `libu`, `hubu`, `gongbu`,
  `xingbu`, `bingbu`, `libu_hr` und `taizi`. `edict-core` kennt neun dieser
  Rollen (`crates/edict-core/src/agents.rs:12-67`), `libu_hr` und `taizi`
  fehlen dort.

## Position

Die Gateway-Entscheidung DEC-GATEWAY-001 trägt. ZeptoClaw ist das Gateway,
`zeptoclaw agent -m` die Aufrufform. Der Aufruf braucht aber eine Ersetzung
für das `--agent`-Schalter, und die genau-einmal-Zusage des Legacy ist im
Rust-Bestand weder erreicht noch erreichbar, solange kein Claim im Datenbank
liegt.

Die Rollenidentität bei ZeptoClaw entsteht aus zwei Quellen: dem Template
über `--template` und der `SOUL.md` im Arbeitsverzeichnis. Beide müssen
explizit gesetzt werden, sonst hängt die Rolle am Konfigurationszustand des
Hosts. Der Worker setzt deshalb je Rolle ein Template und ein
Arbeitsverzeichnis mit `SOUL.md`, setzt `ZEPTOCLAW_AGENTS_DEFAULTS_WORKSPACE`
explizit und startet den Prozess mit diesem Verzeichnis als Arbeitsordner.

Ein Auftrag wird genau einmal ausgeführt, wenn drei Bedingungen zugleich
gelten:

1. Die Datenbank vergibt den Auftrag an genau einen Worker. Dafür braucht es
   eine Dispatchtabelle mit Spalte für den Auftrag, den Versuch, den
   Inhaber, die Lease und den Zustand. Die Auswahl läuft über `FOR UPDATE SKIP
   LOCKED`, eine abgelaufene Lease wird neu vergeben.
2. Der Aufruf ist mit einem Idempotenzschlüssel versehen. Der Schlüssel
   enthält `task_id` und Versuch. Er geht als `EDICT_DISPATCH_ID` an den
   Agenten und steht in der Nutzlast von `agent.output`.
3. Die API nimmt das Ergebnis genau einmal an. Eine eindeutige Spalte auf
   dem Dispatchschlüssel verhindert, dass ein zweites Ergebnis denselben
   Auftrag abschließt.

Der Redis-Strom mit Konsumentengruppe und ACK liefert Zustellung mindestens
einmal. Er kann das nicht ändern, weil der Outbox-Relay und der Legacy
selbst nach einem Commit-Fehler erneut veröffentlichen.

## Dispatch-Ablauf

1. Die API schreibt `task.dispatch.request` über `request_dispatch`
   (`task_store.rs:754-772`). Die Aufgabe wird gesperrt, Ereignis und
   Outbox entstehen in einer Transaktion (`:156-194`).
2. Der Outbox-Relay veröffentlicht das Ereignis in den Strom
   `edict:stream:task.dispatch` (`outbox.rs:47-103`, `redis_bus.rs:86-119`).
3. Der Worker liest mit `XREADGROUP` aus der Gruppe `dispatcher`
   (`redis_bus.rs:142-167`). Offene Einträge holt er sich regelmäßig mit
   `XAUTOCLAIM` und einer Ruhezeit über dem Aufrufzeitlimit (`:183-207`).
4. Der Worker beansprucht den Auftrag in der Datenbank. Schlägt der Claim
   fehl, quittiert er den Eintrag ohne Ausführung.
5. Der Worker schreibt `agent.dispatch.start` mit Erzeuger `dispatcher` und
   dem Thema `agent.heartbeat` (`service.rs:206-217`), transaktional
   zusammen mit dem Outbox-Eintrag.
6. Der Worker startet `zeptoclaw agent --template <rolle> --no-stream -m
   <nachricht>` mit gesetzter Arbeitsumgebung, mit `EDICT_TASK_ID`,
   `EDICT_TRACE_ID`, `EDICT_LEGACY_ID` und `EDICT_DISPATCH_ID`, mit
   `cwd` auf dem Rollenverzeichnis und mit dem Zeitlimit aus
   `EDICT_DISPATCH_TIMEOUT_SEC` (`config.rs:89`).
7. Der Worker schreibt `agent.output` mit Erzeuger `agent.<rolle>`, dem
   Thema `agent.thoughts`, `return_code` und dem Ausgabetext
   (`service.rs:190-203`). Das Ereignis entsteht auch bei Fehlschlag.
8. Erst danach quittiert der Worker den Stromeintrag (`redis_bus.rs:170-180`).
9. Der Agent meldet seine Arbeit über den HTTP-Weg der API, nicht über den
   Strom. Die Antwort erscheint als Fortschritts- und Zustandsänderung am
   Auftrag.

## Risiken und Gegenmaßnahmen

**Doppelte Ausführung durch zu kurze Ruhezeit.** Der Legacy holt offene
Einträge nach 60 Sekunden, der Aufruf darf 300 Sekunden laufen
(`dispatch_worker.py:72-74` gegen `:172`). Ein laufender Auftrag wird
parallel erneut gestartet. Gegenmaßnahme: Ruhezeit über dem Aufrufzeitlimit
und zusätzlich die Lease in der Datenbank. Beide Mechanismen müssen
zusammenpassen, der Datenbank-Claim allein genügt nicht, weil ein zweiter
Worker den Auftrag erst nach Ablauf der Lease erneut bekommt.

**Verlust durch Kürzung.** Der Strom trägt höchstens 10.000 Einträge
(`redis_bus.rs:20`). Ein Auftrag, der nicht rechtzeitig gelesen wird, fällt
aus dem Strom. In der Datenbank bleibt das Ereignis. Ein Rückweg aus der
Datenbank in den Strom fehlt. Gegenmaßnahme: Der Worker prüft regelmäßig
Ereignisse der Datenbank, die kein `agent.output` besitzen, und stellt sie
erneut ein.

**Ereignis und Outbox nicht gemeinsam.** `append_event` schreibt ohne
Transaktion (`task_store.rs:805-812`). Fällt der Prozess zwischen beiden
Inserts aus, liegt das Ereignis in der Datenbank, aber nie im Strom, und
kein Outbox-Eintrag erinnert daran. Gegenmaßnahme: Der Worker schreibt
beide Dispatch-Ereignisse über einen neuen transaktionalen Weg wie
`request_dispatch`.

**Zwei Erzeuger für dasselbe Ereignis.** Der Legacy erzeugt
`task.dispatch.request` aus dem Orchestrator und aus dem Task-Dienst, der
Port kennt nur den Task-Dienst (`service.rs:184` gegen
`orchestrator_worker.py:135` und `task_service.py:178`). Ein Kunde, der nach
Erzeuger filtert, sieht je nach Herkunft unterschiedliche Werte.
Gegenmaßnahme: Für den Port genau einen Erzeuger festlegen. Am ehesten
`orchestrator` für automatische Zustandswechsel und `task_service` für
ausdrückliche Anfragen, mit derselben Nutzlast.

**Die Rollen können nicht zurückmelden.** Die `SOUL.md` verlangt den Aufruf
von `scripts/kanban_update.py` mit der Kennung `JJC-xxx`
(`/home/openchamber/dump/coding/edict-2.0/agents/hubu/SOUL.md:24-43` und
`:61-78`). Das Skript existiert im Legacy
(`/home/openchamber/dump/coding/edict-2.0/scripts/kanban_update.py`). Im
Rust-Port gibt es keine Entsprechung, die Suche über `crates` findet keine
Zeile `kanban_update`, und `crates/edict-cli/src/main.rs` ist ein
Platzhalter. Ohne Board-Befehle bleibt der Auftrag beim Agenten und der
Zustand wechselt nicht. Gegenmaßnahme: Vor dem ersten produktiven Dispatch
einen HTTP-basierten Ersatz bereitstellen und die `SOUL.md` der Rollen
umschreiben.

**Dokumentation driftet.** `README.md:179-180` nennt `OPENCLAW_BIN` und
`OPENCLAW_PROJECT_DIR`, die Konfiguration nutzt `ZEPTOCLAW_BIN` und
`ZEPTOCLAW_PROJECT_DIR` (`config.rs:76` und `:79`). Das README von ZeptoClaw
nennt `--stream` (`README.md:19`), die CLI-Definition kennt nur `--no-stream`
(`src/cli/mod.rs:73-75`). Gegenmaßnahme: Die Aufrufform aus der
CLI-Definition abschreiben, das README von Edict auf `ZEPTOCLAW_*` ziehen.

**Rollenverzeichnis im Bestand.** `edict-core` kennt neun Rollen
(`agents.rs:12-67`), der Legacy hat elf `SOUL.md`. `libu_hr` und `taizi`
fehlen im Kern. Gegenmaßnahme: Rollenverzeichnis als Datei im Repository
führen und in beide Crates prüfen.

## Fehlerklassifikation

| Fehler | Erkennung | Behandlung |
| --- | --- | --- |
| ZeptoClaw nicht installiert | Fehler beim Start, kein Prozess | wiederholen bis `MAX_DISPATCH_RETRY` (`config.rs:84`), dann Auftrag sichtbar blockieren |
| Unbekanntes Template | Exitcode 1 und Meldung `Template '...' not found` (`src/cli/common.rs:105-109`) | nicht wiederholen, Konfigurationsfehler, Auftrag blockieren |
| Aufruf überschreitet das Zeitlimit | Abbruch nach `EDICT_DISPATCH_TIMEOUT_SEC` (`config.rs:89`) | wiederholen, solange die Versuche reichen, sonst blockieren |
| Agent antwortet mit Exitcode ungleich 0 | Rückgabewert des Prozesses | wiederholen, weil der Agent selbst den Fehler melden kann, sonst blockieren |
| Gateway-Dienst nicht erreichbar | `ZEPTOCLAW_HEALTH_URL` antwortet nicht (`config.rs:77`) | wiederholen mit Abstand |
| Redis nicht erreichbar | Fehler in `read_group` oder `ack` (`redis_bus.rs:164` und `:178`) | wiederholen, der Auftrag bleibt im Strom |
| Datenbank nicht erreichbar | Fehler im Claim oder im Ereignis | wiederholen, ohne Quittierung |
| Auftrag während der Ausführung abgebrochen | Zustand `Cancelled` oder `Blocked` in der Datenbank | nicht wiederholen, Ergebnis als verworfen kennzeichnen |
| Nachricht nicht lesbar oder leer | Prüfung vor dem Start | nicht wiederholen, Auftrag blockieren |

Jeder Fehler braucht einen sichtbaren Zustand. Die Zustände dafür stehen im
Schema (`0001_init.sql:15-18`): `Blocked` für einen Auftrag, der auf einen
Eingriff wartet, `Cancelled` für einen Auftrag, der endgültig aufgegeben
wird. Der Worker schreibt zu jedem Fehlschlag ein `agent.output` mit
`return_code` und einem gekürzten Fehlertext und danach ein Zustandsereignis.
Ohne Zustandsänderung bliebe der Auftrag im alten Zustand stehen.

## Empfehlung

1. Aufrufform im Worker festlegen als
   `zeptoclaw agent --template <rolle> --no-stream -m <nachricht>`. Kein
   `--agent`.
2. Je Rolle ein Template unter `~/.zeptoclaw/templates` und ein
   Rollenverzeichnis mit `SOUL.md` bereitstellen. Das Arbeitsverzeichnis
   setzt der Worker über `ZEPTOCLAW_AGENTS_DEFAULTS_WORKSPACE`, damit das
   Verhalten nicht vom Konfigurationszustand des Hosts abhängt.
3. Den Erzeugervertrag unverändert übernehmen: `task_service` für
   `task.dispatch.request`, `dispatcher` für `agent.dispatch.start`,
   `agent.<rolle>` für `agent.output`. Für den automatischen Versand aus
   Zustandswechseln einen eigenen Erzeuger `orchestrator` ergänzen.
4. Eine Dispatchtabelle mit Lease und Versuch anlegen. Der Claim läuft über
   `FOR UPDATE SKIP LOCKED`. Der Idempotenzschlüssel aus `task_id` und
   Versuch geht als `EDICT_DISPATCH_ID` an den Agenten und steht in der
   Nutzlast von `agent.output`. Die API nimmt ein Ergebnis nur einmal an.
5. `agent.dispatch.start` und `agent.output` transaktional über Ereignis und
   Outbox schreiben, nicht über `append_event`.
6. Einen Reclaim-Loop mit `XAUTOCLAIM` betreiben. Die Ruhezeit liegt über
   dem Aufrufzeitlimit.
7. Fehler nach der Tabelle oben trennen. Nach dem letzten Versuch den
   Zustand `Blocked` setzen und den Grund im Ereignis nennen.
8. Vor dem ersten produktiven Dispatch einen Board-Befehl über HTTP
   bereitstellen und die `SOUL.md` der Rollen darauf umstellen.
9. `README.md` auf `ZEPTOCLAW_BIN` und `ZEPTOCLAW_PROJECT_DIR` ziehen.

## Votum

Genehmigt mit Auflagen. DEC-GATEWAY-001 bleibt gültig. Der Worker darf ohne
die Punkte 1 bis 7 nicht produktiv laufen, weil der Aufruf sonst an der
Rolle scheitert, die Zustellung mehrfach ausführt und Fehler ohne sichtbaren
Zustand bleiben. Punkt 8 blockiert den produktiven Betrieb der Agenten, weil
die Rollen ihre Ergebnisse sonst nicht melden können.

## Umsetzung

Offen. Die Auflagen 1 bis 9 gehören in `RUST-PORT-WORKER-001`. Der
Worker-Crate ist beim Stand dieser Debatte ein Platzhalter
(`crates/edict-worker/src/main.rs`).
