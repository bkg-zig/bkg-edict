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

## Laden

Der Import läuft über die Kommandozeile und schreibt über die
HTTP-Schnittstelle des Servers:

```bash
edict migrate import-json --file data/tasks_source.json --dry-run
edict migrate import-json --file data/tasks_source.json
```

Der erste Befehl zeigt nur an, was importiert würde. Der zweite schreibt
die Aufgaben. Ein laufender Server ist dafür nötig.

Im Container liegt die Datei unter `/app/data/tasks_source.json`, weil das
Verzeichnis `data` des Projekts dorthin eingebunden wird.

## Ohne Server laden

Wer die Datei nur prüfen will, öffnet sie auf dem Rechner, nicht im
Container:

```bash
python3 -m json.tool data/tasks_source.json
```

Der Server liest dieselbe Datei über `LEGACY_TASKS_FILE`, wenn er beim Start
den Altbestand übernehmen soll. Der Pfad steht in `.env` und zeigt im
Container auf `/app/data/tasks_source.json`.
