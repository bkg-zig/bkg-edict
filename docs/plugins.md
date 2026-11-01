# Plugins

Ein Plugin ist ein eigener Prozess, den der Host zur Laufzeit startet,
verwaltet und beendet. Der Host muss dafür nicht neu gebaut werden. Die
Kommunikation läuft über JSON-RPC 2.0 auf stdio.

Das Protokoll steht in `crates/edict-plugin-protocol`. Der Plugin-Host liegt
in `crates/edict-plugin-host`. Dort liegen Module für das Manifest, die
sichere Umgebung, die Zustände, die Ports, den Prozessstart, den Zugriff auf
ein einzelnes Plugin, die Registry der geladenen Plugins, die
HTTP-Weiterleitung und die Dienste des Hosts. Der Einstieg in `lib.rs`
bindet diese Module ein. Die Beschreibung unten ist der vereinbarte
Vertrag. Einige Teile sind im Host noch nicht an den Server gebunden.

## Rahmen

Jede Nachricht wird von einem `Content-Length`-Kopf eingeleitet. Der Kopf
trägt die Länge der Nutzlast in Bytes. Danach folgt die JSON-Nachricht.
Diese Rahmung ist eindeutig, auch wenn die Nutzlast Zeilenumbrüche
enthält. Eine Nutzlast über 8 MiB wird abgewiesen.

Auf stdout gehören nur Rahmen. Jede andere Ausgabe auf stdout ist ein
Fehler im Plugin und führt zum Abbruch. stderr wird als Protokoll gelesen.

## Methoden

| Methode | Richtung | Art | Zweck |
| --- | --- | --- | --- |
| `initialize` | Host zu Plugin | Anfrage | Übergibt Protokollversion, Hostversion, Pluginverzeichnis und nicht geheime Konfiguration |
| `health` | Host zu Plugin | Anfrage | Liefert den Zustand `ready`, `degraded` oder `failed` |
| `event` | Host zu Plugin | Hinweis | Liefert ein Edict-Ereignis aus den Abonnements des Plugins |
| `invoke` | beide Richtungen | Anfrage | Führt eine benannte Fähigkeit aus, etwa `gitclaw.review` |
| `emit_event` | Plugin zu Host | Anfrage | Meldet ein Domainereignis, das der Host in Datenbank und Outbox anlegt |
| `shutdown` | Host zu Plugin | Anfrage | Beendet das Plugin geordnet und mit Frist |
| `log` | Plugin zu Host | Hinweis | Schreibt einen Protokolleintrag in das Host-Protokoll |

Der Host öffnet das Gespräch mit `initialize`. Die Antwort nennt
`protocolVersion`, `name`, `version`, `capabilities`, `subscriptions` und
optional `http_base_url`. Der Host nimmt nur Plugins an, deren Manifest und
Antwort dieselbe Protokollversion nennen. Gültig ist derzeit die Version 1.

Die Antwort auf `health` nennt `state`, `uptime_seconds`, `queued_events`
und optional `dropped_events` sowie `last_error`.

## Fehlercodes

| Code | Bedeutung |
| --- | --- |
| `-32601` | Unbekannte Methode |
| `-32602` | Ungültige Parameter oder inkompatible Protokollversion |
| `-32000` | Fehler im Plugin |
| `-32001` | Überlastete Warteschlange |

## Zustellung der Ereignisse

Der Host holt die Ereignisse über den Port `EventSource`. Der Server setzt
diesen Port mit Redis um. Vorgesehen ist je Plugin eine Konsumentengruppe
`plugin:<name>` aus dem Redis-Stream des jeweiligen Themas. Eine angenommene
Nachricht wird mit `XACK` quittiert. Nachrichten eines abgestürzten
Prozesses holt der Host mit `XAUTOCLAIM` zurück. Der Host filtert die
Ereignisse nach den Abonnements des Plugins und übergibt nur den
Nutzlastanteil. Diese Redis-Anbindung ist im Plugin-Host noch nicht
gebunden.

## Grenzen und Sicherheit

- Der Kindprozess startet mit einer reduzierten Umgebung. Der Host übergibt
  nur `PATH`, `LANG` und `HOME`. `HOME` zeigt auf das Pluginverzeichnis.
- Das Plugin bekommt weder das Admin-Token noch Zugangsdaten der Datenbank.
- Der Kindprozess ruft `setsid` auf und läuft damit in einer eigenen
  Sitzung. Läuft er nach der Frist nicht von selbst aus, beendet der Host
  ihn mit einem Signal.
- Die Warteschlqueue eines Plugins fasst 128 Nachrichten. Ist sie voll,
  verwirft der Host das Ereignis, zählt es und meldet den Zustand
  `degraded`. Der Zustand kehrt nach der Leerung der Warteschlange auf
  `ready` zurück.
- Jede Anfrage bekommt ein Zeitlimit. Der Standardwert für Aufrufe an ein
  Plugin liegt bei 300 Sekunden, für das geordnete Beenden bei 5 Sekunden.
- Ein Plugin darf nur auf `127.0.0.1` oder `localhost` lauschen. Der Host
  leitet die vom Plugin gemeldeten Routen weiter und reicht Rohkörper und
  Header unverändert durch. Ein einzelner Rohkörper ist auf 8 MiB begrenzt.
- Der Befehl eines Plugins wird nur innerhalb des Pluginverzeichnisses
  aufgelöst. Absolute Pfade und Pfade außerhalb des Verzeichnisses werden
  abgewiesen. Es gibt keine Shell.

## Manifest

Ein Manifest nennt Name, Version, Protokollversion, Fähigkeiten und die
Ereignisarten, die das Plugin empfangen möchte. Der Host lädt das Manifest
beim Start und bei jedem Austausch neu. Der Austausch eines Plugins
bekommt einen neuen Prozess, während der Host weiterläuft.

## Die beiden Plugins

`edict-plugin-gitclaw` prüft Merge-Anfragen. Der Plan umfasst einen
Webhook-Eingang für GitHub, GitLab und Bitbucket, Sperrregeln für
Entwürfe, Bot-Autoren, doppelte Prüfsummen und die Zeichenfolge
`[skip review]`, strukturierte Befunde, eine Zusammenfassung mit
Inline-Kommentaren, mehrere Anbieter für die Prüfung, statische Analyse
und eine Konfiguration je Repository.

`edict-plugin-edict` unterstützt Mehragenten-Workflows. Der Plan umfasst
die Initialisierung eines Projekts, die Synchronisation eingebetteter
Workflow-Dokumente, `doctor`-Prüfungen, Agentenschleifen mit Journal,
Protokollbefehle für Zustandsübergänge, ein Review-Gate, die Koordination
über Beads, Ereignisprotokolle und ein Hook-System.

Das gitclaw-Crate enthält Module für die Verbindung zum Host, die
Auswertung der Webhook-Ereignisse, den HTTP-Eingang und die Prüfung der
Webhook-Signatur. Das edict-Crate enthält noch nichts außer einem leeren
`main`. Die Protokollanbindung des gitclaw-Plugins ist im Werkzeug noch
nicht verdrahtet.

## Abweichung von der ersten Aufgabenliste

Die ursprüngliche Aufgabenliste nannte die Methoden `plugin/health`,
`plugin/shutdown`, `plugin/webhook`, `edict/dispatch` und
`plugin/dispatch.result`. Das Crate `edict-plugin-protocol` führt statt
dessen die sieben Methoden der Tabelle oben. Verbindlich ist der Stand des
Crates, weil Host und Plugin beide das Crate benutzen. Die beiden Listen
müssen vor der Implementierung des Hosts zusammengeführt werden.
