# Demodaten

`tasks_source.json` enthält vier erfundene Aufgaben im Format des Altbestands.
Die Datei ist **nur Beispieldaten**. Sie beschreibt keine realen Vorgänge,
Personen oder Organisationen und darf nicht als Produktivbestand importiert
werden.

## Inhalt

Die vier Aufgaben tragen die Legacy-Kennungen `JJC-20260224-001`,
`JJC-20260225-001`, `JJC-20260225-002` und `JJC-20260226-001`. Sie zeigen die
Zustände `Done`, `Doing` und `Review`, ein ausführliches Zustandsprotokoll
und eine archivierte Aufgabe. Alle sichtbaren Texte sind ins Deutsche
übersetzt. Die Zustandswerte bleiben in der Schreibweise, die die Datenbank
erwartet.

Die Felder entsprechen genau den Feldern der Datei im Altbestand. Geändert
sind nur die sichtbaren Texte, darunter Titel, Amtsbezeichnungen,
Zustandsprotokoll und Abnahmekriterien. Ein Vorgang ohne Archivierung trägt
die Felder `archived` und `archivedAt` gar nicht.

## Laden

Ein Import über die Kommandozeile existiert im Repository noch nicht. Die
Datei dient als Vorlage für den späteren Import. Im Container liegt sie unter
`/app/data/tasks_source.json`, weil das Verzeichnis `data` des Projekts
dorthin eingebunden wird. Der Server liest dieselbe Datei über
`LEGACY_TASKS_FILE`.

## Datei prüfen

Wer die Datei nur prüfen will, öffnet sie auf dem Rechner, nicht im
Container:

```bash
python3 -m json.tool data/tasks_source.json
```

Der Befehl ist ein Werkzeug für die Prüfung auf dem Rechner. Der Betrieb
selbst braucht kein Python.
