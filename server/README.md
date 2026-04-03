# Elidune Server (Rust)

Library management system — JSON REST API written in Rust (Axum, PostgreSQL, Redis, optional Meilisearch).

## Features

- Catalog (bibliographic records, specimens, circulation)
- Patrons and loans
- Z39.50 import
- JWT authentication, Argon2 passwords
- OpenAPI/Swagger at `/swagger-ui`

## License

[GNU Affero General Public License v3.0](LICENSE) (AGPL-3.0). If you run a modified version as a network service, AGPL obligations apply — see the license text.

## Prerequisites

- **Rust:** stable toolchain (see `Cargo.toml` / CI; typically latest stable)
- **PostgreSQL:** 14+
- **Redis:** required at runtime (Z39.50 cache and related features)
- **Optional:** [Meilisearch](https://www.meilisearch.com/) for catalog full-text search (PostgreSQL fallback if omitted from config)

## Configuration model

The server loads **one TOML file** passed on the command line:

```bash
elidune-server --config /path/to/config.toml
```

Edit `config/default.toml` (or a copy) for database URL, JWT secret, Redis URL, email, Meilisearch, and rate limits.  
Environment variables **do not** replace TOML for `database.url` or `users.jwt_secret` — set those in the file.  
`RUST_LOG` is supported (tracing), and `.env` is loaded if present for local development.

## Run with Docker (API + PostgreSQL + Redis + Meilisearch)

From the **repository root**:

```bash
docker compose -f docker/docker-compose.yml up -d --build
```

- API: `http://localhost:8080` (Swagger: `http://localhost:8080/swagger-ui`)
- Compose uses `docker/config.docker.toml` (mounted read-only). **Change `jwt_secret` and database passwords** before production.
- Optional tools profile: `docker compose -f docker/docker-compose.yml --profile tools up -d` (includes pgAdmin on port 5050).

To rebuild only after changing the Rust code:

```bash
docker compose -f docker/docker-compose.yml build app
docker compose -f docker/docker-compose.yml up -d
```

### API-only image (build locally)

The `docker/Dockerfile` builds **only** the Elidune server binary (no UI). Build from the repo root:

```bash
docker build -f docker/Dockerfile -t elidune-server:local .
```

Run (example):

```bash
docker run --rm -p 8080:8080 \
  -v /absolute/path/to/your/config.toml:/app/config/default.toml:ro \
  elidune-server:local
```

The image default command is `elidune-server --config /app/config/default.toml`.

### Prebuilt “complete” image (API + UI + DB stack)

CI publishes a **full stack** image (Rust API + web UI + PostgreSQL + Redis + Nginx inside one image) to GitHub Container Registry when `main` is pushed:

- **Image:** `ghcr.io/jcollonville/elidune-complete:latest` (and a tag per commit SHA)
- **Build definition:** [`.github/workflows/docker-publish.yml`](.github/workflows/docker-publish.yml), [`docker/Dockerfile.complete`](docker/Dockerfile.complete)

Deployment and host-level proxy examples for that stack: **[README-docker.md](README-docker.md)**.

## Build and run from source

```bash
git clone https://github.com/elidune/elidune-server.git
cd elidune-server
```

Install SQLx CLI (for migrations):

```bash
cargo install sqlx-cli --no-default-features --features postgres,rustls
```

Create the database and user (example names match `config/default.toml`):

```bash
export DATABASE_URL="postgres://elidune:elidune@localhost:5432/elidune"
sqlx migrate run
```

Start Redis locally, then run the server with your config path:

```bash
cargo build --release
./target/release/elidune-server --config config/default.toml
```

Development:

```bash
cargo run -- --config config/default.toml
```

Tests:

```bash
cargo test --lib
# Integration tests need PostgreSQL + Redis (see CI workflow)
export DATABASE_URL="postgres://elidune:elidune@localhost:5432/elidune_test"
sqlx migrate run
cargo test --test '*' -- --nocapture
```

## Reverse proxy (HTTPS / Nginx / Apache)

Put Elidune behind Nginx or Apache for TLS and a stable public URL. Examples (single API upstream, split API + UI, headers, timeouts): **[docs/reverse-proxy.md](docs/reverse-proxy.md)**.

## Database initialization

### Docker Compose

The compose file provisions PostgreSQL; migrations run automatically when the app starts.

### Existing PostgreSQL

Create role and database, then run migrations from your machine:

```bash
export DATABASE_URL="postgres://elidune:elidune@localhost:5432/elidune"
sqlx migrate run
```

See sections above for Docker-on-host and manual install examples if you need step-by-step SQL.

### Default administrator

After migrations, an administrator account exists (change the password immediately in production):

| Field        | Value   |
|-------------|---------|
| Login       | `admin` |
| Password    | `admin` |

## API quick reference

- **Swagger UI:** `http://localhost:8080/swagger-ui`
- **OpenAPI JSON:** `http://localhost:8080/api-docs/openapi.json`

```bash
# Login
curl -s -X POST http://localhost:8080/api/v1/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username": "admin", "password": "admin"}'

# Authenticated call (replace TOKEN)
curl -s "http://localhost:8080/api/v1/items?title=tolkien" \
  -H "Authorization: Bearer TOKEN"
```

## Migration from the legacy system

```bash
python scripts/migrate_data.py \
  --source-db "postgres://old_user:old_pass@old_host/old_db" \
  --target-db "postgres://elidune:elidune@localhost/elidune"
```

Sample legacy data flow is described in older docs if you maintain `scripts/sample_legacy_data.sql`.

## Project layout

```
src/
├── api/          # HTTP handlers
├── models/       # Data types
├── repository/   # SQL access
├── services/     # Business logic
├── marc/         # MARC translation
├── config.rs
├── error.rs
└── main.rs
```

## Additional documentation

- [Full Docker “complete” deployment](README-docker.md)
- [Reverse proxy: Nginx & Apache](docs/reverse-proxy.md)

## Public release checklist (maintainers)

- [ ] Version in `Cargo.toml` matches release notes.
- [ ] `LICENSE` matches intended terms (AGPL-3.0).
- [ ] No secrets in `config/*.toml` committed to the repo.
- [ ] `sqlx migrate run` succeeds on a clean database.
- [ ] `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test` pass (and integration job with Postgres/Redis as in CI).

## Authors

- Package metadata: see `Cargo.toml`. Contributors: [GitHub contributors](https://github.com/elidune/elidune-server/graphs/contributors).

## History

- **Current (Rust)** — JSON REST API, OpenAPI, PostgreSQL, Redis, optional Meilisearch.
- Earlier iterations — legacy C/XML-RPC stack (see project history in your archive if applicable).
