# Debatte: Plugin-Transport und Ablage

Datum: 31.10.2026
Bezug: DEC-PLUGIN-001
Beteiligte Fachleute: Plugin-Transport, Server-Vertrag, Worker-Dispatch

## Fragestellung

Wie sollen Hotplug-Plugins in Edict 2.0 angebunden werden? Der Vorschlag lautete:
Plugins sind eigene Prozesse, sprechen JSON-RPC 2.0 über stdio mit
Content-Length-Rahmung, liegen unter `crates/plugins/`, werden über ein Manifest
zur Laufzeit registriert und erhalten weder Admin-Token noch Datenbankverbindung.
Erstes Plugin ist ein stdio-Adapter für den Volldienst gitclaw.

## Belege aus dem Bestand

- `crates/edict-core/src/repository.rs` trennt `EventPublisher`, `EventRepository`
  und `TaskRepository`. Der Host kann Ereignisse in den Bus schreiben, ohne
  dem Plugin den Bus zu geben.
- `crates/edict-db/src/task_store.rs` schreibt Ereignis und Outbox-Eintrag in
  einem Aufruf. Ein Plugin-Ereignis nimmt denselben Weg wie ein Kernereignis.
- `crates/edict-db/src/redis_bus.rs` bietet Streams mit Konsumentengruppen und
  `XAUTOCLAIM` für abgestürzte Konsumenten. Das ist der zustandbehaftete
  Zustellweg.
- `crates/edict-core/src/config.rs` legt `EDICT_ADMIN_TOKEN`, `DATABASE_URL` und
  `REDIS_URL` in derselben Umgebung ab, die ein Kindprozess sonst erbt. Hier
  muss die Isolation technisch erzwungen werden.
- `crates/edict-server/src/main.rs` war beim Stand der Debatte ein Platzhalter.
  Hotplug war noch nicht implementiert.
- Im Legacy gibt es zwei andere Erweiterungsmechanismen: dateibasierte Skills
  in `dashboard/server.py` und einen Hook-Mechanismus in `rust/crates/edict-hooks`,
  der ein Plugin als Kommando aufruft und die Nutzlast auf stdin schickt.
  Das Legacy-Backend hatte keinen Webhook-Eingang.
- gitclaw ist ein Next.js-Dienst mit eigener PostgreSQL-Instanz, Job-Queue und
  öffentlichen Webhook-Adressen. Er kann als stdio-Kindprozess nicht laufen.
  Die Neuimplementierung in Rust ist die tragfähige Variante.

## Position Plugin-Transport

Der Transport trägt die Prozessgrenze und den Hotplug-Anspruch. Die Entscheidung
ist unvollständig, nicht falsch.

Hotplug verlangt zwei Dinge, die die Entscheidung noch nicht nannte. Der Host
muss Axum-Routen zur Laufzeit anbieten können. In Axum 0.8 wird der Router beim
Start gebaut, für nachgeladene Plugins braucht es eine feste Route wie
`/api/plugins/{name}/{*rest}`, die zur Anfragezeit die Registry befragt. Und die
Ereigniszustellung braucht einen langlebigen Kanal je Plugin. Pub/Sub verliert
Ereignisse, wenn der Host kurz nicht läuft. Der Host liest je Plugin eine
Konsumentengruppe `plugin:<name>`, quittiert nach bestätigtem Zustellen und
holt bei Absturz über `XAUTOCLAIM` zurück.

Laden und Entladen sind Host-Vorgänge und keine JSON-RPC-Methoden. Sie bestehen
aus Prozessstart mit Handshake und aus Shutdown mit Terminierung.

## Methodenliste

| Methode | Richtung | Art | Zweck |
| --- | --- | --- | --- |
| `initialize` | Host zu Plugin | Request | Protokollversion, Hostversion, Pluginverzeichnis, nicht geheime Konfiguration, Grenzwerte. Antwort nennt Protokollversion, Namen, Version, Fähigkeiten, Ereignisabonnements, optionale HTTP-Basis-Adresse |
| `health` | Host zu Plugin | Request | Zustand `ready`, `degraded` oder `failed`, Zeit seit Start, letzter Fehler, Warteschlangenlänge |
| `event` | Host zu Plugin | Notification | ein Edict-Ereignis, gefiltert nach den Abonnements des Plugins |
| `invoke` | beide Richtungen | Request | eine benannte Fähigkeit ausführen, etwa `gitclaw.review` oder `task.lookup` |
| `emit_event` | Plugin zu Host | Request | ein Domainereignis. Der Host vergibt `event_id` und legt Ereignis und Outbox an |
| `shutdown` | Host zu Plugin | Request | geordnetes Beenden mit Frist |
| `log` | Plugin zu Host | Notification | Protokolleintrag für das Host-Log, mit Schwärzung |

Fehlercodes: `-32601` für unbekannte Methode, `-32602` für ungültige Parameter
oder inkompatible Protokollversion, `-32000` für Fehler im Plugin, `-32001` für
Überlast.

## Risiken und Gegenmaßnahmen

**Credential-Leak.** Der Kindprozess erbt sonst die ganze Umgebung des Hosts.
Gegenmaßnahme: Prozess mit leerer Umgebung starten und nur eine Positivliste
übergeben. Geheimnisse des Plugins liegen in einer Datei, die nur das Plugin
liest. Host-Logs schwärzen markierte Geheimnisse, Rahmeninhalte werden nicht
protokolliert.

**Zombie- und Kindprozesse.** Ein Plugin kann eigene Kinder starten.
`kill_on_drop` beendet nur den direkten Kind. Gegenmaßnahme: Plugin in eine
eigene Prozessgruppe starten, die Gruppe nach Frist toten, danach `wait`
aufrufen. Das Muster mit Timeout, `kill` und `wait` steht im Legacy unter
`rust/crates/edict-runtime/src/process.rs`.

**Backpressure.** Ein blockierter Plugin-Prozess darf den Host nicht aufhalten.
Gegenmaßnahme: je Plugin eine begrenzte Warteschlange mit Vorgabe 128, ein
Zähler für verworfene Nachrichten, ein Höchstmaß pro Rahmen, ein Zeitlimit je
Anfrage und eine Obergrenze für laufende Anfragen. Ist die Warteschlange voll,
meldet der Host `degraded` und stellt die Zustellung an dieses Plugin
temporär ein.

**Versionierung.** `protocolVersion` als ganze Zahl im Manifest und in
`initialize`. Der Host nimmt nur bekannte Versionen an und beendet den Prozess
mit `-32602` bei Abweichung.

**Protokollreinheit.** Auf stdout gehören nur Rahmen. Alles andere auf stdout
ist ein Defekt im Plugin und führt zum Abbruch. stderr wird als Log gelesen.

**Manifest und Kommando.** Der Befehl wird nur innerhalb des Pluginverzeichnisses
aufgelöst. Keine Shell, keine absoluten Pfade außerhalb des Verzeichnisses.

**HTTP-Eingang.** Ein Plugin darf nur auf `127.0.0.1` lauschen. Der Host leitet
die vom Plugin deklarierten Routen weiter und reicht Rohkörper und Header
unverändert durch, damit das Plugin die Signatur des Anbieters selbst prüfen
kann. Das Geheimnis des Anbieters bleibt beim Plugin.

**Datenabfluss über Ereignisse.** Der Host filtert Ereignisse nach den
Abonnements des Plugins und übergibt nur den Nutzlastanteil.

## Abgrenzung zu Skills und Hooks

Skill ist eine Datei, die das Verhalten eines Agenten beschreibt. Hook ist ein
einmaliger Kommandoaufruf mit Nutzlast auf stdin. Plugin ist ein langlebiger
Prozess mit eigenem HTTP-Eingang. Der Edict-Mehragenten-Workflow läuft als
Hook-Mechanismus und nicht über den Plugin-Host.

## Votum

Genehmigt mit Auflagen. Die Auflagen für die Umsetzung sind:

1. Feste Regel für den HTTP-Eingang von Plugins.
2. Ein gemeinsames Protokoll-Crate, das Host und Plugins benutzen.
3. Die sieben Methoden mit Protokollversion.
4. Der Redis-Stream statt Pub/Sub als Zustellweg.
5. Das Verbot des Erbens der Host-Umgebung.

Ohne diese Punkte ist das gitclaw-Plugin nicht umsetzbar. Der Kern der
Entscheidung bleibt gültig.

## Umsetzung

- `crates/edict-plugin-protocol` trägt Rahmen, Nachrichtentypen,
  Protokollversion 1, die sieben Methoden und die Fehlercodes.
- `crates/edict-plugin-host` verwaltet Prozessstart, Handshake, Zustände,
  Warteschlangen, Aufrufe und Beenden.
- `crates/plugins/gitclaw` und `crates/plugins/edict` sprechen dieses Protokoll.
