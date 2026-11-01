# syntax=docker/dockerfile:1
#
# Betriebsimage für Edict 2.0.
#
# Der Builder kompiliert die vier Binaries des Projekts. Das Laufzeitimage
# enthält nur die Binaries, die CA-Zertifikate und die dynamischen
# Bibliotheken. Python ist im Image nicht vorhanden.

# ---------------------------------------------------------------- Builder ---
FROM rust:1.83-bookworm AS builder

RUN apt-get update \
    && apt-get install --no-install-recommends --yes \
        libssl-dev \
        pkg-config \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Manifest und Lockfile zuerst, damit der Abhängigkeitsaufbau gecacht bleibt.
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

# Die Oberfläche (crates/edict-web) gehört nicht in dieses Image. Sie wird
# separat für wasm32 gebaut und zur Laufzeit unter /app/web eingebunden.
RUN cargo build \
        --release \
        --locked \
        --bin edict-server \
        --bin edict-worker \
        --bin edict \
        --bin edict-plugin-gitclaw

# --------------------------------------------------------------- Laufzeit ---
FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install --no-install-recommends --yes \
        ca-certificates \
        libgcc-s1 \
        libssl3 \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system --gid 10001 edict \
    && useradd --system --uid 10001 --gid edict --home-dir /app --shell /usr/sbin/nologin edict

WORKDIR /app

COPY --from=builder /build/target/release/edict-server /usr/local/bin/edict-server
COPY --from=builder /build/target/release/edict-worker /usr/local/bin/edict-worker
COPY --from=builder /build/target/release/edict /usr/local/bin/edict
COPY --from=builder /build/target/release/edict-plugin-gitclaw /usr/local/bin/edict-plugin-gitclaw

# /app/data trägt das dateibasierte Gedächtnis. /app/web nimmt die gebaute
# Oberfläche als Bind-Mount auf, damit das Image ohne Node und ohne Dioxus
# auskommt.
RUN mkdir --parents /app/data /app/web \
    && chown --recursive edict:edict /app

ENV BACKEND_HOST=0.0.0.0 \
    BACKEND_PORT=8080 \
    LEGACY_DATA_DIR=/app/data \
    LEGACY_TASKS_FILE=/app/data/tasks_source.json

EXPOSE 8080

USER edict

CMD ["edict-server"]
