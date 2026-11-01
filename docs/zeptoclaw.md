# Anbindung von ZeptoClaw

ZeptoClaw ist das Gateway, über das Edict die Agenten aufruft. Das Programm
gehört nicht zum Repository und nicht zum Container-Image. Es ist eine
extern bereitzustellende Abhängigkeit.

## Konfiguration

Der Kern liest drei Variablen aus der Umgebung.

| Variable | Vorgabe | Bedeutung |
| --- | --- | --- |
| `ZEPTOCLAW_BIN` | `zeptoclaw` | Programm, mit dem der Worker das Gateway aufruft |
| `ZEPTOCLAW_HEALTH_URL` | `http://127.0.0.1:9090/health` | Adresse, an der der Worker die Bereitschaft prüft |
| `ZEPTOCLAW_PROJECT_DIR` | ohne | Verzeichnis der ZeptoClaw-Sitzungen |

Ohne Wert für `ZEPTOCLAW_PROJECT_DIR` nutzt das Gateway sein eigenes
Standardverzeichnis. Die Konfiguration von ZeptoClaw selbst liegt im
Verzeichnis `~/.zeptoclaw` des Benutzers, unter dem das Programm läuft.

## Aufruf

Der Worker übergibt den Auftrag eines Agenten als Nachricht an den
Gateway-Aufruf:

```bash
zeptoclaw agent --template <rolle> --no-stream -m <nachricht>
```

Der Schalter `--template` wählt eine Agentenvorlage aus dem eingebauten
Angebot oder aus einer Datei unter `~/.zeptoclaw/templates`. Die Vorlage
bestimmt die Rolle des Agenten. Der Schalter `--no-stream` unterdrückt die
fortlaufende Ausgabe, damit der Worker nur die fertige Antwort verarbeitet.
Der Schalter `--mode` legt die Rechte des Agenten fest, etwa `observer`,
`assistant` oder `autonomous`. Der Schalter `--dry-run` zeigt nur die
Werkzeugaufrufe, ohne sie auszuführen.

Um die Bereitschaft zu prüfen, fragt der Worker die Adresse aus
`ZEPTOCLAW_HEALTH_URL` ab. Ist das Gateway nicht erreichbar, bleibt der
Worker ohne Dispatch und meldet den Zustand im Protokoll.

## Ereignisse

Der Worker schreibt vor dem Aufruf das Ereignis `agent.dispatch.start` und
nach dem Aufruf das Ereignis `agent.output`. Der Rückgabewert des Aufrufs
geht in das Ereignis ein. Beide Ereignisse entstehen in derselben Transaktion
wie die Änderung am Aufgabenzustand, damit die Ereigniskette vollständig
bleibt.

## Grenzen

| Variable | Vorgabe | Wirkung |
| --- | --- | --- |
| `EDICT_DISPATCH_TIMEOUT_SEC` | `300` | Zeitlimit für einen Aufruf |
| `MAX_DISPATCH_RETRY` | `3` | Zahl der erneuten Versuche |
| `EDICT_MAX_CONCURRENT_DISPATCH` | `3` | Zahl gleichzeitiger Aufrufe |
| `STALL_THRESHOLD_SEC` | `180` | Sekunden ohne Fortschritt bis zum Stillstandsverdacht |

Ein Aufruf, der das Zeitlimit überschreitet, wird beendet. Der Worker
schreibt das Ergebnis des Abbruchs in die Outbox, damit der Vorgang nicht
stillschweigend hängen bleibt.

## Befehle von ZeptoClaw

Die folgenden Befehle gehören zu ZeptoClaw und nicht zu Edict. Sie sind
für die Einrichtung und die Fehlersuche nützlich.

| Befehl | Zweck |
| --- | --- |
| `zeptoclaw onboard` | Richtet Konfiguration und Arbeitsverzeichnis ein |
| `zeptoclaw doctor` | Prüft das System, mit `--online` auch die Anbieter |
| `zeptoclaw status` | Zeigt den Systemzustand |
| `zeptoclaw version` | Zeigt die Version |
| `zeptoclaw agent --template <rolle> --no-stream -m <text>` | Führt einen Auftrag aus |
| `zeptoclaw agent-stdin` | Führt Aufträge über stdin und stdout aus |
| `zeptoclaw batch --input <datei>` | Verarbeitet mehrere Aufträge aus einer Datei |
| `zeptoclaw gateway` | Startet das Gateway, optional mit `--containerized` |
| `zeptoclaw daemon` | Startet einen überwachten Dienst |
| `zeptoclaw panel` | Startet die Bedienoberfläche |
| `zeptoclaw serve` | Startet einen HTTP-Dienst im OpenAI-Format |
| `zeptoclaw migrate --from <pfad>` | Übernimmt Konfiguration und Skills aus einer OpenClaw-Installation |
| `zeptoclaw update --check` | Prüft auf eine neue Version |
| `zeptoclaw uninstall` | Entfernt den Zustand von ZeptoClaw |

Der Befehl `batch` liest `.txt`, `.json` oder `.jsonl` und schreibt das
Ergebnis mit `--output` in eine Datei. Der Befehl `migrate` bietet
`--dry-run` für eine Vorschau.

## Offene Punkte

- Das Container-Image enthält kein ZeptoClaw. Der Worker findet das
  Programm nur, wenn es auf dem Host oder als zusätzlicher Dienst im
  Compose-Netz bereitsteht. Die Compose-Datei stellt es bisher nicht bereit.
- Die Konfigurationstabelle der `README.md` nennt für das Programm und das
  Arbeitsverzeichnis andere Variablennamen als der Kern. Der Kern liest
  `ZEPTOCLAW_BIN` und `ZEPTOCLAW_PROJECT_DIR`. Die Betriebsdatei folgt dem
  Kern. Die `README.md` ist zu berichtigen.
