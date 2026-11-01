#!/usr/bin/env bash
#
# Baut die Weboberflaeche und legt das Ergebnis in crates/edict-web/dist ab.
#
# Das Verzeichnis dist ist der Bind-Mount der Compose-Datei. Der Server
# liefert daraus die Oberflaeche aus.
#
# Aufruf:
#
#     ./build.sh          # Release-Build
#     ./build.sh debug    # Build fuer die Entwicklung

set -euo pipefail

cd "$(dirname "$0")"

if ! command -v dx >/dev/null 2>&1; then
  echo "Das Werkzeug dx fehlt. Es wird mit 'cargo install dioxus-cli --version 0.7.10' installiert." >&2
  exit 1
fi

PROFIL="${1:-release}"
case "$PROFIL" in
  release)
    dx build --platform web --release
    QUELLE="target/dx/edict-web/release/web/public"
    ;;
  debug)
    dx build --platform web
    QUELLE="target/dx/edict-web/debug/web/public"
    ;;
  *)
    echo "Unbekannter Profilname: $PROFIL. Erlaubt sind release und debug." >&2
    exit 1
    ;;
esac

if [ ! -f "$QUELLE/index.html" ]; then
  echo "Der Build hat kein Ergebnis unter $QUELLE erzeugt." >&2
  exit 1
fi

rm -rf dist
mkdir -p dist
cp -r "$QUELLE/." dist/

echo "Die Oberflaeche liegt in $(pwd)/dist."
