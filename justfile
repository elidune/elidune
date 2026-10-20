# Local shortcuts mirroring .github/workflows/ci.yml
# Run from repo root: `just`, `just ci`, `just ci-ui`, …

default:
    @just --list

rust := "1.90"
compose := "docker/docker-compose.yml"
database_url := "postgres://elidune:elidune@localhost:5432/elidune_test"
redis_url := "redis://localhost:6379"

export DATABASE_URL := database_url
export REDIS_URL := redis_url
export CI := "true"

# --- Test infrastructure (Postgres 16 + Redis 7, same as CI) ---

# Start db + redis from docker/docker-compose.yml
ci-services-up:
    docker compose -f {{compose}} up -d db redis

# Create elidune_test if missing (CI uses POSTGRES_DB=elidune_test)
ci-test-db-create:
    docker compose -f {{compose}} exec -T db psql -U elidune -d postgres -tc \
      "SELECT 1 FROM pg_database WHERE datname = 'elidune_test'" \
      | grep -q 1 \
      || docker compose -f {{compose}} exec -T db psql -U elidune -d postgres -c \
      "CREATE DATABASE elidune_test;"

ci-test-deps: ci-services-up ci-test-db-create

ci-services-down:
    docker compose -f {{compose}} stop db redis

# Pin matches CI (sqlx 0.7 line; 0.9 needs rustc 1.94+)
install-sqlx:
    cargo install sqlx-cli --version 0.7.4 --locked --no-default-features --features postgres

# --- Server (lint-server + test-fast + test-db) ---

server-fmt:
    cd server && cargo +{{rust}} fmt --all -- --check

server-fmt-fix:
    cd server && cargo +{{rust}} fmt --all

server-clippy:
    cd server && cargo +{{rust}} clippy --all-targets --all-features -- -D warnings

server-openapi:
    cd server && cargo +{{rust}} test --lib api::openapi::tests::openapi_schema_compiles -- --exact

server-test-lib:
    cd server && cargo +{{rust}} test --lib -- --nocapture

server-lint: server-fmt server-clippy server-openapi

server-migrate:
    cd server && sqlx migrate run

server-sqlx-prepare:
    cd server && cargo sqlx prepare -- --lib

server-sqlx-check:
    cd server && SQLX_OFFLINE=true cargo +{{rust}} check --lib

server-test-integration:
    cd server && cargo +{{rust}} test --test '*' -- --nocapture

# Full test-db job (needs sqlx-cli + running ci-test-deps)
server-test-db: ci-test-deps server-migrate server-sqlx-prepare server-sqlx-check server-test-integration

server-ci: server-lint server-test-lib server-test-db

# --- UI (lint-ui + build-ui + test-ui) ---

ui-install:
    cd ui && pnpm install --frozen-lockfile

ui-lint:
    cd ui && pnpm lint

ui-test:
    cd ui && pnpm test

ui-build:
    cd ui && pnpm build

ui-ci: ui-install ui-lint ui-test ui-build

# --- Docker (docker-build job on PR) ---

docker-build-server:
    docker build -f docker/Dockerfile .

docker-build-ui:
    docker build -f docker/Dockerfile.ui .

docker-build-all-in-one:
    docker build -f docker/Dockerfile.all-in-one .

docker-ci: docker-build-server docker-build-ui docker-build-all-in-one

# --- Aggregates ---

# Everything CI runs on a typical PR (server + UI, no coverage)
ci: ui-ci server-lint server-test-lib server-test-db

# Subsets
ci-ui: ui-ci
ci-server: server-ci
ci-server-lint: server-lint
ci-server-fast: server-lint server-test-lib
