# ── Build stage ───────────────────────────────────────────────────────────────
# Pinned to the exact project toolchain (see rust-version in Cargo.toml).
FROM rust:1.96.1-slim AS build
WORKDIR /app

# Resolve dependencies in their own layer so source-only changes don't
# re-download the whole crate set. `cargo fetch` requires the package to have
# a target, so a stub main.rs is created first; the real source overwrites it.
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src && echo 'fn main() {}' > src/main.rs
RUN cargo fetch

COPY . .
# `cargo build --strip` is still unstable, so symbols are stripped via RUSTFLAGS.
RUN RUSTFLAGS="-C strip=symbols" cargo build --release

# ── Runtime stage ─────────────────────────────────────────────────────────────
FROM debian:bookworm-slim

# curl is only used by the HEALTHCHECK; ca-certificates for any TLS work.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

# Run as an unprivileged user.
RUN useradd --create-home --uid 10001 oxideforms

# Defaults; override with -e / compose `environment:` (real env vars win over
# any mounted .env, since dotenvy never overrides variables already set).
ENV HOST=0.0.0.0 \
    PORT=3000 \
    FORMS_DIR=/forms \
    DB_PATH=/data/forms.db

WORKDIR /app
COPY --from=build /app/target/release/forms /usr/local/bin/forms

# Mount points: form definitions (read + hot-reloaded) and the SQLite database.
RUN mkdir -p /forms /data && chown oxideforms:oxideforms /forms /data

# The entrypoint fixes ownership of the mounted data dir (the container starts
# as root so it can) and then drops to the unprivileged user via setpriv.
COPY docker-entrypoint.sh /usr/local/bin/docker-entrypoint.sh
RUN chmod +x /usr/local/bin/docker-entrypoint.sh

VOLUME ["/forms", "/data"]
EXPOSE 3000
HEALTHCHECK --interval=30s --timeout=3s --start-period=10s \
    CMD curl -fsS http://127.0.0.1:3000/healthz || exit 1

ENTRYPOINT ["/usr/local/bin/docker-entrypoint.sh"]
CMD ["forms"]
