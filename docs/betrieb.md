# Betrieb

## Dienste

Der Stack besteht aus vier Diensten.

| Dienst | Image | Aufgabe |
| --- | --- | --- |
| `postgres` | `postgres:16-alpine` | Führende Datenquelle für Aufgaben und Ereignisse |
| `redis` | `redis:7-alpine` | Ereignisbus mit Streams und Pub/Sub |
| `server` | `edict:local` | HTTP-Schnittstelle, WebSocket und Plugin-Host |
| `worker` | `edict:local` | Outbox-Relay, Dispatcher und Scheduler |

Server und Worker benutzen dasselbe Image. Beide starten erst, wenn
PostgreSQL und Redis ihre Prüfung bestanden haben. Im Redis läuft kein
`allkeys-lru`, weil die Streams keine Cache-Daten tragen, sondern die
Arbeitsteilung. Das AOF-Verfahren hält die Redis-Daten nach einem Neustart
des Containers.

## Start

```bash
cp .env.example .env
docker compose build
docker compose up -d
docker compose logs -f server worker
```

Nur die Datenbank und der Ereignisbus lassen sich ohne Image starten:

```bash
docker compose up -d postgres redis
```

Das Builder-Image lässt sich über die Variable `RUST_IMAGE` wechseln. Das ist
nötig, solange die veröffentlichte `Cargo.lock` Crates mit der Ausgabeedition
2024 nennt, die Cargo 1.83 nicht auflösen kann:

```bash
RUST_IMAGE=rust:1.90-bookworm docker compose build
```

Der HTTP-Port ist auf dem Host über `EDICT_HTTP_PORT` erreichbar, im
Standard unter `http://localhost:8080`. Der Container lauscht auf Port 8080.
Die Vorgabe in `edict-core` nennt für `BACKEND_PORT` den Wert 8000, deshalb
setzt `.env` den Wert 8080 ausdrücklich.

## Oberfläche

Die Oberfläche wird nicht in das Server-Image gebaut. Sie entsteht als
Dioxus-Anwendung für das Ziel `wasm32-unknown-unknown` und braucht dafür
Node 20 und das Werkzeug `dx` in Version 0.7.10:

```bash
cd crates/edict-web
dx build --platform web
```

Das Ergebnis liegt danach in `crates/edict-web/dist`. Die Compose-Datei
bindet dieses Verzeichnis als `/app/web` in den Container ein. Der Server
liefert die Dateien statisch aus. Das Image enthält weder Node noch das
Werkzeug `dx`, damit es klein bleibt und keine zweite Laufzeitumgebung
benötigt.

## Umgebung

Die Datei `.env` entsteht aus `.env.example`. Alle Variablen des Kerns
stehen dort mit deutscher Erklärung.

| Variable | Vorgabe | Bedeutung |
| --- | --- | --- |
| `POSTGRES_HOST` | `localhost` | Host der Datenbank |
| `POSTGRES_PORT` | `5432` | Port der Datenbank |
| `POSTGRES_DB` | `edict` | Name der Datenbank |
| `POSTGRES_USER` | `edict` | Benutzer der Datenbank |
| `POSTGRES_PASSWORD` | `edict_secret_change_me` | Passwort der Datenbank |
| `DATABASE_URL` | aus `POSTGRES_*` | Vollständige Verbindungsangabe |
| `REDIS_URL` | `redis://localhost:6379/0` | Verbindung zum Ereignisbus |
| `BACKEND_HOST` | `0.0.0.0` | Bindeadresse des Servers |
| `BACKEND_PORT` | `8000` | Port des Servers |
| `SECRET_KEY` | `change-me-in-production` | Geheimnis für interne Nachrichten |
| `DEBUG` | `false` | Ausführliche Fehlermeldungen |
| `ZEPTOCLAW_BIN` | `zeptoclaw` | Programm für den Aufruf der Agenten |
| `ZEPTOCLAW_HEALTH_URL` | `http://127.0.0.1:9090/health` | Prüfadresse des Gateways |
| `ZEPTOCLAW_PROJECT_DIR` | ohne | Verzeichnis der ZeptoClaw-Sitzungen |
| `LEGACY_DATA_DIR` | `../data` | Verzeichnis des dateibasierten Gedächtnisses |
| `LEGACY_TASKS_FILE` | `../data/tasks_source.json` | Datei der Aufgaben im Legacy-Format |
| `STALL_THRESHOLD_SEC` | `180` | Sekunden ohne Änderung bis zum Stillstandsverdacht |
| `MAX_DISPATCH_RETRY` | `3` | Zahl der erneuten Zustellversuche |
| `EDICT_ALLOWED_ORIGINS` | `http://localhost:7891` | Allowlist der erlaubten Herkunft |
| `EDICT_ADMIN_TOKEN` | ohne | Token für die Adminrouten |
| `EDICT_DISPATCH_TIMEOUT_SEC` | `300` | Sekunden bis zum Abbruch eines Dispatchs |
| `EDICT_MAX_CONCURRENT_DISPATCH` | `3` | Zahl gleichzeitiger Dispatchs |
| `RUST_IMAGE` | `rust:1.83-bookworm` | Image des Builders, nur für `docker compose build` |

Die Compose-Datei setzt `DATABASE_URL`, `REDIS_URL`, `BACKEND_HOST`,
`BACKEND_PORT`, `LEGACY_DATA_DIR` und `LEGACY_TASKS_FILE` für beide
Anwendungsdienste passend zum Netz der Container. Die Werte in `.env`
gelten für einen Betrieb ohne Docker.

## Datenbank und Migrationen

Das Schema liegt in `crates/edict-db/migrations/0001_init.sql`. Die
Migration legt die Tabellen `tasks`, `todos`, `thoughts`, `events` und
`outbox` an. Die Migration läuft beim Start des Servers.

Vor dem ersten Produktivimport ist eine Sicherung zu erstellen:

```bash
docker compose exec postgres pg_dump -U edict -Fc edict > edict.dump
```

Wiederherstellung:

```bash
docker compose exec -T postgres pg_restore -U edict -d edict --clean --if-exists < edict.dump
```

Der Import der Aufgaben aus dem Altbestand ist im Repository noch nicht
implementiert. Die Demodaten liegen in `data/tasks_source.json` und werden
nach `/app/data/tasks_source.json` eingebunden. Die Datei `LEGACY_TASKS_FILE`
im Container verweist auf diesen Pfad.

## Beobachtung

Die Dienste liefern ihre Zustände über `docker compose ps` und ihre
Protokolle über `docker compose logs`. Der Server beantwortet die Prüfung
unter `/healthz`, die Adminrouten unter `/api/admin/health/deep` prüfen
Datenbank, Redis und Outbox. Der Worker schreibt fehlgeschlagene
Zustellversuche in die Spalten `attempts` und `last_error` der Outbox.

Ein steigender Wert in `attempts` deutet auf einen Fehler im Relay hin. Ein
wachsender Bestand unpublished Einträge in der Outbox bedeutet, dass der
Worker läuft und der Redis nicht erreichbar ist.

## Sicherheit

- Der Server läuft im Image als Benutzer `edict` ohne Shell.
- Die Zugangsdaten stehen in `.env` und nicht im Image.
- Passwörter und Tokens erscheinen nicht in Protokollen und Fehlermeldungen.
- Die Allowlist `EDICT_ALLOWED_ORIGINS` lässt keine freie Herkunft zu.
- Plugins laufen als getrennte Prozesse und erben weder das Admin-Token
  noch die Datenbankverbindung des Kerns.
- Redis läuft ohne Verdrängung, damit keine Zustellung verloren geht.

## Offene Punkte

Diese Punkte betreffen den aktuellen Stand des Repositorys. Sie sind nicht
im Betriebsdokument versteckt.

- `edict-server` enthält nur einen Platzhalter ohne `main`-Funktion. Der
  Befehl `docker compose build` scheitert deshalb an der Übersetzung dieses
  Crates.
- `edict-core`, `edict-db`, `edict-plugin-protocol`, `edict-plugin-host`,
  `edict-worker` und `edict-cli` übersetzen mit einem Werkzeug ab Rust 1.85.
  Der Builder nutzt Rust 1.83.
- `crates/edict-web` enthält bisher nur eine Probeanwendung. Das Verzeichnis
  `crates/edict-web/dist` fehlt. Docker legt es beim Start des Servers leer
  an, bis der Web-Build vorliegt.
- Die Konfigurationstabelle der `README.md` nennt für das Programm und das
  Arbeitsverzeichnis andere Variablennamen als der Kern. Der Kern liest
  `ZEPTOCLAW_BIN` und `ZEPTOCLAW_PROJECT_DIR`. Die Betriebsdatei folgt dem
  Kern.
- Die Binary `edict` liest `EDICT_API_URL` selbst. `edict-core` kennt die
  Variable nicht, weil sie nur den Server betrifft.
- Der Kern liest `STALL_THRESHOLD_SEC`, verwendet den Wert aber nicht. Die
  Grenzen für den Herzschlag einer Aufgabe stehen fest in
  `compute_heartbeat` in `edict-core`.
- Die veröffentlichte `Cargo.lock` nennt Crates mit der Ausgabeedition 2024.
  Cargo 1.83 aus dem Builder-Image kann sie nicht auflösen. Der Bau läuft
  mit einem neueren Image über die Variable `RUST_IMAGE`. Die Lockdatei
  selbst zu ändern liegt außerhalb dieses Dokuments.
