# ============================================================================
# Xerboxion core — image ALPHA (host WASM + bus PLC + ploxions UI)
# Tourne comme un service de la stack Project N.O.M.A.D. Sert le xer (l'OS) et
# les ploxions web-*/ même-origine. Offline-first : aucune dépendance réseau au
# runtime (TLS via rustls/ureq, pas d'OpenSSL).
# Build : depuis le fork, `docker build -f xerboxion/Dockerfile xerboxion`.
# ============================================================================

# ---- étage build : compile le binaire Rust du host ----
FROM rust:1-bookworm AS builder
WORKDIR /build
# ring (rustls) a besoin d'un compilateur C ; pkg-config par prudence
RUN apt-get update && apt-get install -y --no-install-recommends build-essential pkg-config \
    && rm -rf /var/lib/apt/lists/*
COPY core/ ./core/
WORKDIR /build/core
RUN cargo build --release -p xerboxion-host
# rassemble les ploxions UI (web-*) en préservant l'arborescence (le glob COPY l'aplatirait)
RUN mkdir -p /assets && cp -r web-* /assets/

# ---- étage runtime : slim, juste le binaire + les assets ----
FROM debian:bookworm-slim AS runtime
WORKDIR /app
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
# binaire du host (le bin du crate s'appelle xerboxion-rt)
COPY --from=builder /build/core/target/release/xerboxion-rt /usr/local/bin/xion
# ploxions UI servis depuis l'assets_dir : <assets>/web-<id>/ -> /px/<id>/
COPY --from=builder /assets/ /app/ploxions/
RUN mkdir -p /data/state
# token d'écriture du store (vide = lectures publiques, écritures fermées) — à fixer au déploiement
ENV XER_STORE_TOKEN=""
EXPOSE 8730
# 0.0.0.0 pour être joignable dans le réseau de la compose
CMD ["xion","serve","/app/ploxions","--addr","0.0.0.0","--port","8730","--state-dir","/data/state"]
