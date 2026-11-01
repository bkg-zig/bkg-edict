# Edict-Web

Weboberflaeche von Edict 2.0. Die Anwendung laeuft als WebAssembly im Browser und
spricht mit dem Edict-Server ueber dessen HTTP-Schnittstelle und WebSocket.

## Aufbau

| Datei | Inhalt |
| --- | --- |
| `src/main.rs` | Einstiegspunkt, registriert die Module und startet Dioxus |
| `src/app.rs` | Gemeinsamer Zustand, Befehlskanal, Hintergrundschleifen, Kopfzeile |
| `src/api.rs` | HTTP-Aufrufe, WebSocket-Nachrichten, Fehlertexte |
| `src/models.rs` | Datenmodelle, kanonische Zustandswerte, deutsche Beschriftungen |
| `src/views/board.rs` | Board mit den Spalten nach Zustand |
| `src/views/task_detail.rs` | Detailansicht mit Aktionen, Teilaufgaben und Protokollen |
| `src/views/overview.rs` | Agentenstatus, Statistik, Morgenbericht, Modellwechsel |
| `src/components/` | Aufgabenkarte, Filterleiste, Formular, Badges, Meldungen |
| `src/styles.rs` | CSS mit dunkler und heller Darstellung |

Die Oberflaeche nutzt `gloo-net` fuer HTTP und WebSocket. Die Aufgabenliste
kommt aus `/api/live-status`. Aenderungen meldet der Server ueber das
WebSocket unter `/ws` mit Nachrichten der Form `{"type":"event","topic":…,"data":…}`.
Faellt die Verbindung aus, fragt die Oberflaeche die Liste alle fuenf Sekunden
ueber HTTP ab.

## Warum eine eigene Kopie der Datenmodelle

`edict-core` laesst sich nicht fuer `wasm32-unknown-unknown` uebersetzen. Die
Abhaengigkeit `fs2 0.4.3` nutzt Unix-spezifische Systemaufrufe, die es im
WASM-Ziel nicht gibt. Der Uebersetzer bricht mit folgendem Fehler ab:

```
error[E0433]: failed to resolve: use of undeclared crate or module `sys`
 --> crates/edict-core/src/filestore.rs
```

`src/models.rs` enthaelt deshalb eine eigene Kopie von `TaskState`, `Task` und
den Beschriftungen. `edict-core` bleibt als Dev-Abhaengigkeit erhalten, damit
die Zustandswerte in den Tests gegen die kanonische Quelle geprueft werden
koennen.

## Voraussetzungen

- Rust 1.85 oder neuer mit dem Ziel `wasm32-unknown-unknown`
  (`rustup target add wasm32-unknown-unknown`)
- Dioxus CLI 0.7.10 (`cargo install dioxus-cli --version 0.7.10`)

## Bauen

```bash
cargo build --target wasm32-unknown-unknown
```

Das erzeugt die WebAssembly-Datei unter `target/wasm32-unknown-unknown/debug/`.

Das vollstaendige Paket mit HTML, CSS und JavaScript entsteht mit der Dioxus CLI:

```bash
dx build --platform web
```

Das Ergebnis liegt danach unter `target/dx/edict-web/debug/web/public/`.
Dort liegen `index.html` und im Unterordner `wasm/` die gebundelte `.js`-Datei
sowie das `.wasm`-Modul. Der Ordner kann direkt hinter einem Webserver
ausgeliefert werden. Mit `dx build --platform web --release` entsteht das
Paket unter `target/dx/edict-web/release/web/public/`.

Das Skript `build.sh` erledigt den Bau und legt das Ergebnis unter `dist/`
ab. Dieses Verzeichnis bindet die Compose-Datei als `/app/web` in den
Container ein.

## Pruefen

```bash
cargo fmt -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo build --target wasm32-unknown-unknown
dx build --platform web
```

## Betrieb

Die Oberflaeche erwartet den Edict-Server unter derselben Adresse wie die
Seite. Alle Routen beginnen mit `/api/`, das WebSocket liegt unter `/ws`.
Mit der Umgebungsvariable `EDICT_API_BASE` laesst sich eine abweichende
Serveradresse zur Bauzeit setzen, zum Beispiel:

```bash
EDICT_API_BASE=https://edict.example.com dx build --platform web
```

Ohne diese Variable wird der Host der Seite verwendet.

## Bedienung

- **Board**: Spalten nach Zustand, Filter fuer Zustand, Organisation und
  Titel, Umschalter fuer offene und archivierte Aufgaben, Sortierung nach
  Aktualisierung oder Titel.
- **Detailansicht**: alle Felder einer Aufgabe, Teilaufgaben, Zustandsprotokoll,
  Scheduler und Aktivitaet. dort stehen die Aktionen zum Weiterschalten,
  Archivieren, Anhalten, Abbrechen, Fortsetzen, Abnehmen und Aufwecken.
- **Uebersichten**: Agentenstatus und -aktivitaet, amtliche Statistik,
  Morgenbericht, Agentenkonfiguration, Protokoll der Modellwechsel und das
  Ergebnis des letzten Modellwechsels.
