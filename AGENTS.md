# AGENTS.md — Elidune monorepo

Elidune is a library management system split into two independently buildable components in one Git repository.

## Layout

| Path | Stack | Guide |
|------|-------|-------|
| `server/` | Rust · Axum · SQLx · PostgreSQL · Redis · Meilisearch | [`server/AGENTS.md`](server/AGENTS.md) |
| `ui/` | React 19 · TypeScript · Vite · TanStack Query | [`ui/AGENTS.md`](ui/AGENTS.md) |
| `docker/` | Multi-stage Docker builds and Compose stacks | [`docs/README-docker.md`](docs/README-docker.md) |

## Cross-cutting rules

- **API contract:** all HTTP routes under `/api/v1`. The UI uses a relative base URL (`/api/v1`) — same-origin via nginx in production, Vite proxy in dev.
- **Do not serve the SPA from Axum.** Static assets are built from `ui/` and served by nginx (`docker/nginx-ui.conf` or `docker/nginx-all-in-one.conf`).
- **Monorepo commits:** a single change set may touch `server/` and `ui/` together; keep API and UI contract changes in sync.
- **Docker builds:** always use the **repository root** as context (`docker build -f docker/Dockerfile .`).
- **Package manager (UI):** pnpm only (`ui/pnpm-lock.yaml`).

## Common commands (from repo root)

```bash
# Dev — two terminals
cd server && cargo run -- --config config/default.toml
cd ui && pnpm dev

# Docker full stack
docker compose -f docker/docker-compose.yml up --build

# Lint / test
cd server && cargo fmt --check && cargo clippy && cargo test
cd ui && pnpm lint && pnpm build
```

## Domain vocabulary

- **Biblio** — bibliographic record
- **Item** — physical copy of a biblio (retired synonym: Specimen — do not use in new docs/code)
- **Loan** — circulation record
- **Hold** — reservation queue entry

See the per-component AGENTS.md files for coding conventions, permissions, and i18n rules.
