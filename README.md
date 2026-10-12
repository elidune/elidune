# Elidune

Library management system (LMS) — monorepo containing the Rust REST API and the React SPA.

**Live demo:** [elidune.b-612.fr](https://elidune.b-612.fr/)

## Repository layout

| Path | Description |
|------|-------------|
| [`server/`](server/) | Rust API (Axum, PostgreSQL, Redis, Meilisearch) |
| [`ui/`](ui/) | React 19 + Vite SPA |
| [`docker/`](docker/) | Dockerfiles and Compose stacks |
| [`docs/`](docs/) | Deployment guides and frontend API contracts |

## Quick start (development)

**API** (requires PostgreSQL and Redis).

`server/config/default.toml` is gitignored. Copy the sample and edit it before the first start:

```bash
cd server
cp config/sample.toml config/default.toml
```

Change these keys from `config/sample.toml`. The process exits on startup while the sample placeholders are still in place:

- `users.jwt_secret` — replace `change-this-secret-in-production` with a different secret of at least 32 characters
- `meilisearch.api_key` — replace `changeme` with a key of at least 16 characters, or remove the key (Meilisearch is optional)

`database.url` and `redis.url` are already set to local defaults (`postgres://elidune:elidune@localhost:5432/elidune` and `redis://127.0.0.1:6379`). Point them at your PostgreSQL and Redis if those differ.

```bash
cargo run -- --config config/default.toml
```

**UI** (proxies `/api` to `http://127.0.0.1:8080`):

```bash
cd ui
pnpm install
pnpm dev
```

Open [http://localhost:3000](http://localhost:3000).

## Docker deployment

From the repository root:

```bash
# Full stack: UI + API + PostgreSQL + Redis + Meilisearch
docker compose -f docker/docker-compose.yml up --build -d

# All-in-one (single container, demo / simple install)
docker compose -f docker/docker-compose.all-in-one.yml up --build -d
```

Docker Compose does not read `config/default.toml`. The API starts with no `--config` argument; `ELIDUNE_*` environment variables are the configuration and override a TOML file when one is loaded (`ELIDUNE_` prefix, `__` between nested keys). Set `ELIDUNE_USERS__JWT_SECRET` before the first start (at least 32 characters, not `change-this-secret-in-production`). When Meilisearch is enabled, `ELIDUNE_MEILISEARCH__API_KEY` must be at least 16 characters. Compose already supplies `ELIDUNE_DATABASE__URL` and `ELIDUNE_REDIS__URL` for the bundled services.

See [docs/README-docker.md](docs/README-docker.md) for ports, volumes, GHCR images, and production reverse-proxy setup.

## Architecture

- **Runtime:** nginx serves the SPA and reverse-proxies `/api` to the Axum API (same-origin). The API does not serve static UI assets.
- **API base path:** `/api/v1` (relative URL in the SPA — no build-time API URL required).
- **First setup:** no default admin; the UI wizard drives `POST /api/v1/first_setup`.
- **Auth rate limits:** login, 2FA, password reset, and first setup are capped per client IP (default 30/minute, burst 30) and per login (5 failures / 15 minutes). `GET /auth/me` is not on that strict cap. Behind nginx, set `ELIDUNE_SERVER__TRUSTED_PROXIES` to the proxy CIDR so `X-Forwarded-For` is used only from that proxy. Staff clear a temporary block with `DELETE /api/v1/auth/rate-limit/{login}`.

## Agent / contributor guides

- Server: [`server/AGENTS.md`](server/AGENTS.md)
- UI: [`ui/AGENTS.md`](ui/AGENTS.md)

## License

[GNU Affero General Public License v3.0](LICENSE) (AGPL-3.0).

## Migration from split repositories

This monorepo replaces the former separate repositories:

- `elidune-server-rust` → `server/`
- `elidune-ui` → `ui/`

Those repositories are archived; use this repository for all new work.
