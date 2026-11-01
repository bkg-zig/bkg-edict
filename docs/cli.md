# Kommandozeile

Das Binary heißt `edict` und liegt im Paket `edict-cli`. Es schreibt
ausschließlich über die HTTP-Schnittstelle des Servers. Es hält keine
Datenbankverbindung und liest die Dateien des Datenbestands nicht
unmittelbar.

Der Quellcode von `edict-cli` ist im Repository noch im Aufbau. Die
`main.rs` verweist auf Module, die noch nicht vorhanden sind. Die Befehle
unten sind der im Projekt vereinbarte Satz und noch nicht lauffähig.

## Adresse

Das Binary liest die Basisadresse des Servers aus `EDICT_API_URL`. Ohne
diese Variable greift der Vorgabewert `http://localhost:8080`. Der Wert
steht in `.env.example` und wird beim Start aus `.env` geladen.

## Aufgaben

| Befehl | Wirkung |
| --- | --- |
| `edict task list` | Listet Aufgaben nach Zustand und Filter |
| `edict task show <taskId>` | Zeigt eine Aufgabe mit Zustandsprotokoll |
| `edict task stats` | Zeigt Kennzahlen zu den Aufgaben |
| `edict task live-status` | Zeigt den Live-Status des Boards |
| `edict task archive <taskId>` | Archiviert eine Aufgabe |
| `edict task review <taskId>` | Bestätigt oder weist eine Prüfung zurück |

Die Kennung kann die technische UUID oder die Legacy-Kennung sein. Das
Binary erkennt das Format `JJC-YYYYMMDD-NNN` und sucht dann über die
Legacy-Route.

## Board

| Befehl | Wirkung |
| --- | --- |
| `edict kanban create` | Legt eine Aufgabe an |
| `edict kanban state <taskId> <state>` | Wechselt den Zustand einer Aufgabe |
| `edict kanban flow <taskId>` | Zeigt das Zustandsprotokoll |
| `edict kanban done <taskId>` | Schließt eine Aufgabe ab |
| `edict kanban block <taskId> <reason>` | Blockiert eine Aufgabe |
| `edict kanban todo <taskId>` | Zeigt und schreibt die Teilaufgaben |
| `edict kanban progress <taskId> <text>` | Schreibt einen Fortschrittseintrag |

Ein Zustandswechsel wird gegen die Zustandsmaschine geprüft. Ein
unzulässiger Übergang endet mit einer deutschen Fehlermeldung und ohne
Änderung an der Aufgabe.

## Migration

| Befehl | Wirkung |
| --- | --- |
| `edict migrate import-json --file <datei>` | Liest Aufgaben aus einer Datei im Legacy-Format |
| `edict migrate import-json --file <datei> --dry-run` | Zeigt nur an, was importiert würde |

Der Import legt je Aufgabe ein `task.created`-Ereignis an und lässt den
Scheduler-Zustand unberührt.

## Skills

| Befehl | Wirkung |
| --- | --- |
| `edict skill list` | Listet die Skills eines Agenten |
| `edict skill list-remote` | Listet die entfernten Skills |
| `edict skill add-remote <agentId> <name> <url>` | Übernimmt einen Skill aus einer entfernten Quelle |
| `edict skill update-remote <agentId> <name>` | Aktualisiert einen entfernten Skill |
| `edict skill remove-remote <agentId> <name>` | Entfernt einen entfernten Skill |
| `edict skill check-updates` | Prüft alle entfernten Skills auf neue Fassungen |

## Modelle

| Befehl | Wirkung |
| --- | --- |
| `edict model set <agentId> <model>` | Stellt einen Modellwechsel ein |
| `edict model log` | Zeigt das Protokoll der Modellwechsel |
| `edict model last-result` | Zeigt das Ergebnis des letzten Modellwechsels |

## Morgenbrief

| Befehl | Wirkung |
| --- | --- |
| `edict morning-brief show` | Zeigt den Morgenbrief |
| `edict morning-brief show <datum>` | Zeigt den Morgenbrief eines Tages |
| `edict morning-brief refresh` | Startet die Sammlung erneut |
| `edict morning-brief config` | Zeigt und schreibt die Konfiguration der Sammlung |

## Ausgabe und Fehler

Erfolgreiche Befehle schreiben das Ergebnis als Text oder als JSON, wenn
der Aufrufer das Format verlangt. Fehler enden mit dem Text `Fehler:` und
einer deutschen Meldung. Der Rückgabewert des Prozesses ist dann ungleich
null. Passwörter und Tokens erscheinen nicht in der Ausgabe.
