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

**API** (requires PostgreSQL and Redis):

```bash
cd server
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

See [docs/README-docker.md](docs/README-docker.md) for ports, volumes, GHCR images, and production reverse-proxy setup.

## Architecture

- **Runtime:** nginx serves the SPA and reverse-proxies `/api` to the Axum API (same-origin). The API does not serve static UI assets.
- **API base path:** `/api/v1` (relative URL in the SPA — no build-time API URL required).
- **First setup:** no default admin; the UI wizard drives `POST /api/v1/first_setup`.

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
