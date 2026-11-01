# Kommandozeile

Das Binary heißt `edict` und liegt in dem Paket `edict-cli`. Es schreibt
ausschließlich über die HTTP-Schnittstelle des Servers. Es hält keine
Datenbankverbindung und liest die Dateien des Datenbestands nicht
unmittelbar.

Der Quellcode in `crates/edict-cli` übersetzt. Die Befehle unten sind
umgesetzt. Der Server ist noch kein lauffähiges Binary, deshalb enden die
Aufrufe ohne Server mit einer Fehlermeldung.

## Adresse

Die Adresse des Servers wird in dieser Reihenfolge ermittelt:

1. Die Option `--url` überschreibt alle anderen Quellen.
2. Die Umgebungsvariable `EDICT_API_URL` gilt danach.
3. Die Umgebungsvariable `EDICT_SERVER_URL` gilt als nächstes.
4. Die Konfigurationsdatei `~/.config/edict/config.json` liefert den
   Eintrag `serverUrl`. Das Verzeichnis folgt `XDG_CONFIG_HOME`, sonst dem
   Verzeichnis `.config` des Benutzers.
5. Ohne Treffer gilt die Vorgabe `http://127.0.0.1:8080`.

Der Befehl `edict init` schreibt die Konfigurationsdatei. Für die Adminrouten
liest das Binary das Token aus `EDICT_ADMIN_TOKEN` und sendet es im Kopf
`X-Admin-Token`.

## Aufgaben

| Befehl | Wirkung |
| --- | --- |
| `edict tasks list` | Listet Aufgaben nach Zustand und Filter |
| `edict tasks show <id>` | Zeigt eine Aufgabe mit Zustandsprotokoll |
| `edict tasks create --title <titel>` | Legt eine Aufgabe an |
| `edict tasks archive <id>` | Archiviert eine Aufgabe |
| `edict tasks unarchive <id>` | Holt eine Aufgabe aus dem Archiv |
| `edict task advance <id> --to <zustand> --agent <agent>` | Wechselt den Zustand einer Aufgabe |
| `edict task action <id> --action <aktion>` | Führt eine Aktion auf einer Aufgabe aus |
| `edict task todos <id> --from <json>` | Schreibt die Teilaufgaben einer Aufgabe |
| `edict task progress <id> --agent <agent> --content <text>` | Schreibt einen Fortschrittseintrag |
| `edict task review <id> --action <entscheidung>` | Bestätigt oder weist eine Prüfung zurück |

`edict tasks list` kennt die Schalter `--state`, `--org` und `--archived`.
`--state` nimmt die kanonischen Zustandsnamen und die deutschen Anzeigenamen
an. Ohne `--archived` zeigt das Binary die aktiven Aufgaben.

`edict tasks create` kennt die Schalter `--description`, `--priority` und
`--assignee`. Die Priorität ist `low`, `normal`, `high` oder `urgent`.

`edict task advance` kennt den Schalter `--reason`. Fehlt er, trägt der
Eintrag den Text „Zustandswechsel über die Kommandozeile“. Der Server prüft
den Übergang gegen die Zustandsmaschine und lehnt unzulässige Wechsel mit
einer deutschen Meldung ab.

`edict task action` kennt den Schalter `--payload`. Er erwartet ein
JSON-Objekt mit weiteren Feldern. Die Aktion `stop`, `cancel` und `resume`
sind im CLI vorgesehen.

`edict task progress` sendet zuerst die Route `/api/task-progress`. Antwortet
der Server mit 404, versucht das Binary `/api/task-action`. Bleiben beide
Wege erfolglos, nennt die Fehlermeldung beide Routen.

Die Kennung darf die fachliche Kennung im Format `JJC-YYYYMMDD-NNN`, die
technische Kennung oder die Spur eines Vorgangs sein.

`edict task review` sendet die Felder `taskId`, `action` und `comment` an die
Route `/api/review-action`. Der Schalter `--action` nimmt `approve` oder
`reject` an, der Schalter `--comment` eine Begründung. Ein anderer Wert
bricht den Befehl vor dem Aufruf ab.

## Betrieb

| Befehl | Wirkung |
| --- | --- |
| `edict init` | Schreibt die Konfigurationsdatei und prüft den Server |
| `edict sync` | Lädt den Live-Status und speichert ihn lokal |
| `edict doctor` | Prüft Server, Datenbank und Gateway |
| `edict protocol` | Zeigt den Wire-Vertrag der Serverantworten |

`edict init` überschreibt eine vorhandene Konfigurationsdatei nur mit dem
Schalter `--force`. Der Befehl schreibt die Datei und prüft danach die Route
`/healthz`. Ist der Server nicht erreichbar, bleibt die Konfigurationsdatei
trotzdem bestehen und der Befehl endet mit einem Fehler.

`edict sync` speichert den Live-Status unter
`~/.local/share/edict/status.json`. Der Schalter `--output` nennt eine andere
Zieldatei. Das Verzeichnis folgt `XDG_DATA_HOME`, sonst dem Verzeichnis
`.local/share` des Benutzers.

`edict doctor` fragt die Routen `/healthz`, `/api/live-status` und
`/api/agents-status` ab und zählt die fehlgeschlagenen Prüfungen.

## Agenten und Berichte

| Befehl | Wirkung |
| --- | --- |
| `edict agents status` | Zeigt den Status aller Agenten |
| `edict agents activity <agentId>` | Zeigt die Aktivität eines Agenten |
| `edict brief` | Zeigt den Morgenbrief |
| `edict stats` | Zeigt Kennzahlen zu den Aufgaben |
| `edict skills list` | Listet die entfernten Skills |

`edict brief` kennt den Schalter `--date`. Das Datum steht im Format
`JJJJ-MM-TT` oder `JJJJMMTT`. Ohne Datum fragt das Binary die Route
`/api/morning-brief` ab, sonst die Route `/api/morning-brief/<JJJJMMTT>`.

`edict stats` fragt die Route `/api/officials-stats` ab und ergänzt die
Aufgabenzahl aus dem Live-Status.

## Ausgabe und Fehler

Die Befehle `doctor`, `tasks list`, `tasks show`, `tasks create`,
`tasks archive`, `tasks unarchive`, `task advance`, `task action`,
`task todos`, `task progress`, `task review`, `agents status`,
`agents activity`, `brief`, `stats` und `skills list` kennen den Schalter
`--json`. Er schreibt das Ergebnis der Serverantwort als JSON auf die
Standardausgabe. Ohne den Schalter erscheint eine Tabelle oder eine
Meldung.

Fehler enden mit dem Text `Fehler:` und einer deutschen Meldung auf der
Fehlerausgabe. Der Rückgabewert des Prozesses ist dann ungleich null.
Passwörter und Tokens erscheinen nicht in der Ausgabe.
