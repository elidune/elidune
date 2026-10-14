# Elidune Server (Rust)

Library management system — JSON REST API written in Rust (Axum, PostgreSQL, Redis, optional Meilisearch).

**Live demo:** [elidune.b-612.fr](https://elidune.b-612.fr/) (credentials : admin/admin)

## License

[GNU Affero General Public License v3.0](LICENSE) (AGPL-3.0). If you run a modified version as a network service, AGPL obligations apply — see the license text.


## Why this stack

**Fast and efficient** — The server is written in **Rust** and runs on **Tokio**: predictable latency, low overhead per request, and no garbage-collection pauses. Hot paths stay async end-to-end, from HTTP (**Axum**) to the database and Redis.

**Modern API design** — **JSON** over HTTP/1.1, **JWT** auth, **OpenAPI 3** with **Swagger UI** out of the box, **CORS** for SPAs, and **Server-Sent Events** for live updates. Passwords use **Argon2**; staff accounts can use **2FA (TOTP)**.

**Solid data layer** — **PostgreSQL** with versioned **SQLx** migrations (no opaque ORM — explicit SQL you can review and tune). **Redis** backs caching and protocol-adjacent features. Optional **Meilisearch** adds fast catalog search with a **PostgreSQL** fallback so the system stays usable without it.

**Operable in production** — Structured logging (**tracing**), **health** and **readiness** probes, configurable **rate limiting**, gzip-friendly stack, and a clear split between **static config** (file) and **runtime settings** (database) for day-two changes.

**Free software** — **AGPL-3.0**: you can study, modify, and self-host the stack; see [License](#license).

## Features

### Catalog & metadata

- **Bibliographic records** — CRUD on biblios; link **series** and **collections**; attach **physical items** (copies) with barcodes, call numbers, and circulation flags; **CSV export** of bibliographic lists.
- **Search** — Full-text catalog search via **Meilisearch** when configured, with **PostgreSQL** fallback.
- **Covers** — Resolve cover images by ISBN (public endpoint).
- **Sources** — Manage catalog **sources**, merge duplicates, archive.

### Import & cataloging

- **Z39.50** — Search remote catalogs, import records, **Redis-backed** query cache; configure Z39.50 servers via the API.
- **MARC** — Load MARC into biblios, **batch import** with status tracking; suitable for staff workflows and background jobs.

### Circulation

- **Loans** — Checkout, return, **renew** (by loan or by item), **overdue** listing, **loan settings** (borrow rules).
- **Batch circulation** — **Batch return** and **batch checkout** for efficiency at the desk.
- **Holds / reservations** — Place, list, and cancel copy-level or title-level (biblio) holds.
- **Reminders** — Trigger **overdue reminder** emails (with configured SMTP).
- **MARC export** — Export a patron’s **loan history** as MARC for interlibrary loan or archives.
- **Fines** — Fine rules, list patron fines, **pay** or **waive**; tied to circulation policy.

### Patrons & access

- **Users** — Patron and staff accounts: list, create, update, delete; **account types**; **force password change**.
- **Authentication** — **JWT** access tokens, **Argon2** password hashing; **2FA (TOTP)** with setup/disable and recovery codes; **password reset** and **change password**; **profile** updates for the logged-in user.
- **Public types** — Audience classes (e.g. youth/adult) with **per–media-type loan settings**.

### OPAC & public API

- **OPAC** — Public **search** and **biblio detail** without staff auth; **availability** per copy, plus `onHold`, `expectedReturnDate` while the library-local due date is today or later, and title `holdsCount`. No `dueDate` and no borrower identity.
- **Library info** — Public read of library contact details; staff can update **library information**.

### Onboarding & operations

- **First setup** — No default admin: **`/health`** / **`/ready`** expose `need_first_setup`; **`POST /first_setup`** creates the first administrator and initial settings (typically driven by the **frontend** wizard).
- **Inventory** — **Inventory sessions**: scan barcodes (single or batch), list missing copies, reports, session close.
- **Opening hours & closures** — **Schedules**: periods, time slots, **closures** (holidays, exceptions).
- **Equipment** — Optional **equipment** inventory (non-book assets) with CRUD.
- **Events** — Library **events** CRUD and **announcement** sending (email integration where configured).
- **Visitor counts** — Record and list **visitor statistics** when used.

### Reporting & administration

- **Statistics** — Dashboard-style **stats** (loans, users, catalog), **ad‑hoc queries**, **saved queries** and run-by-id; **schema** discovery for building reports.
- **Audit** — **Audit log** for sensitive actions, with **export**.
- **Admin configuration** — Read/update **runtime settings** (sections in DB), optional **email test**, **search reindex** (Meilisearch).
- **Maintenance & tasks** — **Maintenance** actions; **background tasks** list and status (e.g. MARC batches, long-running jobs).

### Realtime & integration

- **Server-Sent Events** — **`/events/stream`** for live updates to connected clients.
- **Rate limiting** — Strict per-IP limits on login, 2FA, password reset, and first setup, plus a short per-login attempt window. `/auth/me` and `/auth/profile` use a separate per-user quota. See [Auth rate limits](#auth-rate-limits).

### API & docs

- **OpenAPI 3** — **`/swagger-ui`** and **`/api-docs/openapi.json`** (**utoipa**).
- **CORS** — Configurable allowed origins for browser clients.
- **Version** — **`/version`** endpoint for deployment checks.



## Prerequisites

- **Rust:** stable toolchain (see `Cargo.toml` / CI; typically latest stable)
- **PostgreSQL:** 14+
- **Redis:** required at runtime (Z39.50 cache and related features)
- **Optional:** [Meilisearch](https://www.meilisearch.com/) for catalog full-text search (PostgreSQL fallback if omitted from config)

## Configuration model

The server loads an optional TOML file (`--config`), then applies `ELIDUNE_*` environment variables on top. The prefix is `ELIDUNE` and nested keys use `__` (`ELIDUNE_USERS__JWT_SECRET` overrides `users.jwt_secret`). A variable replaces the same key from the file. With no `--config` argument, environment variables are the whole configuration — that is how the Docker images start the API.

Before the first start from source, copy the sample. `config/default.toml` is gitignored (`**/config/default.toml`):

```bash
cp config/sample.toml config/default.toml
```

Change these keys from `config/sample.toml`. Startup refuses the sample placeholders:

| Key | Sample value | What to set |
|---|---|---|
| `users.jwt_secret` | `change-this-secret-in-production` | A different secret, at least 32 characters |
| `meilisearch.api_key` | `changeme` | At least 16 characters, or remove the key (Meilisearch is optional) |

`database.url` (`postgres://elidune:elidune@localhost:5432/elidune`) and `redis.url` (`redis://127.0.0.1:6379`) are local defaults. Point them at the PostgreSQL and Redis you will use.

```bash
elidune-server --config config/default.toml
```

Docker Compose does not mount that file. Set the same values with environment variables, at least `ELIDUNE_USERS__JWT_SECRET` (the compose default is still the rejected placeholder). `ELIDUNE_DATABASE__URL` and `ELIDUNE_REDIS__URL` already point at the bundled services. `ELIDUNE_MEILISEARCH__API_KEY` must be at least 16 characters when set; the compose default (`change-this-meili-key`) meets that length.

`RUST_LOG` is supported (tracing), and `.env` is loaded if present for local development.

### Auth rate limits

| Setting | Env | Default |
|---|---|---|
| `server.trusted_proxies` | `ELIDUNE_SERVER__TRUSTED_PROXIES` | empty (ignore forwarding headers; use the TCP peer) |
| `server.auth_ip_per_minute` | `ELIDUNE_SERVER__AUTH_IP_PER_MINUTE` | 30 |
| `server.auth_ip_burst` | `ELIDUNE_SERVER__AUTH_IP_BURST` | 30 |
| `server.auth_identifier_max_failures` | `ELIDUNE_SERVER__AUTH_IDENTIFIER_MAX_FAILURES` | 5 |
| `server.auth_identifier_window_secs` | `ELIDUNE_SERVER__AUTH_IDENTIFIER_WINDOW_SECS` | 900 |
| `server.auth_session_per_minute` | `ELIDUNE_SERVER__AUTH_SESSION_PER_MINUTE` | 120 |
| `server.auth_session_burst` | `ELIDUNE_SERVER__AUTH_SESSION_BURST` | 60 |
| `server.public_per_minute` | `ELIDUNE_SERVER__PUBLIC_PER_MINUTE` | 600 |
| `server.public_burst` | `ELIDUNE_SERVER__PUBLIC_BURST` | 120 |

`trusted_proxies` is a comma-separated CIDR list (`127.0.0.1/32,::1/128`). The strict IP quota covers login, 2FA, password reset, and first setup, and is sized for several desks behind one NAT. Login and password-reset attempts are also counted per normalized login or email (5 failures / 15 minutes). That block expires on its own. A librarian or admin with users write rights clears it with `DELETE /api/v1/auth/rate-limit/{login}`.

`GET /auth/me` and `PUT /auth/profile` are not on the strict limiter. Their quota is per user id.

Both quotas live in process memory, same as the `tower_governor` IP buckets. Redis is not used for them. A `429` response carries `Retry-After` and `{ "code": "rate_limited", "retryAfter": <seconds> }`.

## Run with Docker (API + PostgreSQL + Redis + Meilisearch)

From the **repository root**:

```bash
docker compose -f docker/docker-compose.yml up -d --build
```

- API: `http://localhost:8080` (Swagger: `http://localhost:8080/swagger-ui`)
- Compose configures the API with `ELIDUNE_*` environment variables (no TOML mount). Set `ELIDUNE_USERS__JWT_SECRET` before the first start; see [Configuration model](#configuration-model).
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

The image command is `elidune-server` with no `--config` flag, so settings come from `ELIDUNE_*` environment variables. To use a file instead, pass `--config` and mount it:

```bash
docker run --rm -p 8080:8080 \
  -v /absolute/path/to/your/config.toml:/app/config/default.toml:ro \
  elidune-server:local \
  --config /app/config/default.toml
```

### Prebuilt all-in-one image (API + UI + DB stack)

CI publishes a **full stack** image (Rust API + web UI + PostgreSQL + Redis + Meilisearch + Nginx inside one image) to GitHub Container Registry when `main` is pushed:

- **Image:** `ghcr.io/elidune/elidune-all-in-one:latest` (and a tag per commit SHA)
- **Build definition:** [`.github/workflows/docker-publish.yml`](.github/workflows/docker-publish.yml), [`docker/Dockerfile.all-in-one`](docker/Dockerfile.all-in-one)

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

Copy the sample config (gitignored destination) and edit `users.jwt_secret`, `meilisearch.api_key`, `database.url`, and `redis.url` before the first start. See [Configuration model](#configuration-model).

```bash
cp config/sample.toml config/default.toml
```

Create the database and user (example names match `config/sample.toml`):

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

### First setup (no default administrator)

There is **no** pre-created administrator after migrations. The **frontend** drives initial setup: it checks **`GET /health`** or **`GET /ready`** for `need_first_setup`, then submits **`POST /api/v1/first_setup`** to create the first admin account and library settings. Until that completes, use the wizard flow rather than logging in.


## API quick reference

- **Swagger UI:** `http://localhost:8080/swagger-ui`
- **OpenAPI JSON:** `http://localhost:8080/api-docs/openapi.json`

```bash
# Login (use credentials from first setup; see docs/first-setup-api-frontend.md)
curl -s -X POST http://localhost:8080/api/v1/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username": "YOUR_ADMIN_LOGIN", "password": "YOUR_PASSWORD"}'

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

- [Full Docker all-in-one deployment](README-docker.md)
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
