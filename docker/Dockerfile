# API-only image — build from monorepo root: docker build -f docker/Dockerfile .
# Toolchain: rust:1.90-bookworm. Config via mounted TOML or ELIDUNE_* env vars (see server/src/config.rs).

FROM rust:1.90-bookworm AS builder

WORKDIR /app

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    git \
    && rm -rf /var/lib/apt/lists/*

COPY server/Cargo.toml ./

# Cache dependency build
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src

COPY server/src ./src
COPY server/migrations ./migrations
COPY server/config ./config
COPY server/data ./data

RUN touch src/main.rs && \
    cargo build --release

FROM debian:bookworm-slim

WORKDIR /app

RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    libssl3 \
    postgresql-client \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/elidune-server /app/elidune-server
COPY --from=builder /app/migrations /app/migrations
COPY --from=builder /app/config /app/config
COPY --from=builder /app/data /app/data

RUN useradd -m -u 1000 elidune && \
    chown -R elidune:elidune /app

USER elidune

EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8080/api/v1/health || exit 1

CMD ["/app/elidune-server"]
