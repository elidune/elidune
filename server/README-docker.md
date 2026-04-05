# Elidune Docker deployment

This guide covers deploying Elidune with **Docker Compose** using either a **multi-container stack** (`docker-compose.yml`) or an **all-in-one** image (`docker-compose.all-in-one.yml`). For TLS and a reverse proxy in front of the API or split API + UI, see [docs/reverse-proxy.md](docs/reverse-proxy.md).

## Table of contents

1. [Choosing a Compose file](#choosing-a-compose-file)
2. [Images built on GitHub](#images-built-on-github)
3. [Configuration with `.env`](#configuration-with-env)
4. [Prerequisites](#prerequisites)
5. [Deploy: all-in-one (`docker-compose.all-in-one.yml`)](#deploy-all-in-one-docker-composeall-in-oneyml)
6. [Deploy: multi-service (`docker-compose.yml`)](#deploy-multi-service-docker-composeyml)
7. [Verification](#verification)
8. [Data and volumes](#data-and-volumes)
9. [Useful commands](#useful-commands)
10. [Troubleshooting](#troubleshooting)
11. [Host Nginx (optional)](#host-nginx-optional)
12. [Deployment checklist](#deployment-checklist)

---

## Choosing a Compose file

| | `docker/docker-compose.yml` | `docker/docker-compose.all-in-one.yml` |
|---|-----------------------------|------------------------------------------|
| **Layout** | One container per concern: API app, PostgreSQL, Redis, Meilisearch (optional `pgadmin` via profile `tools`). | Single container: PostgreSQL, Redis, Meilisearch, API, and Nginx (GUI) via Supervisor. |
| **Image** | Default: **build** from `docker/Dockerfile` as `elidune-server:local`. Alternatively set **`ELIDUNE_IMAGE`** to `ghcr.io/elidune/elidune-server` ([built on GitHub](#images-built-on-github)) and use `up --no-build` after `pull`. | Uses **`ELIDUNE_IMAGE`** (default `elidune-all-in-one:latest`). **Recommended:** pull [from GHCR](#images-built-on-github). |
| **Ports (defaults)** | API `8080`→host `API_PORT` or `8080`; DB `5432`; Redis `6379`; Meilisearch `7700`. | Maps host ports to PostgreSQL, Redis, Meilisearch, API (`8282`→8080), GUI (`8181`→80). |
| **Best for** | Development, scaling or swapping components, or when you already run an external database. | Simple installs on one host, demos, or minimal moving parts. |

From the **repository root** (where this file lives), use:

```bash
# Multi-service stack
docker compose -f docker/docker-compose.yml up -d

# All-in-one
docker compose -f docker/docker-compose.all-in-one.yml up -d
```

(`docker-compose` with a hyphen also works if your installation provides it.)

---

## Images built on GitHub

On every push to `main`, **GitHub Actions** (`.github/workflows/docker-publish.yml`) builds and pushes both images to [GHCR](https://docs.github.com/en/packages/working-with-a-github-packages-registry/working-with-the-container-registry):

| Image | Dockerfile | Example tag |
|-------|------------|-------------|
| `ghcr.io/elidune/elidune-server` | `docker/Dockerfile` | `:latest`, `:<git-sha>` |
| `ghcr.io/elidune/elidune-all-in-one` | `docker/Dockerfile.all-in-one` | `:latest`, `:<git-sha>` |

**All-in-one** — set the image and pull:

```bash
echo "ELIDUNE_IMAGE=ghcr.io/elidune/elidune-all-in-one:latest" >> .env
docker login ghcr.io
docker compose -f docker/docker-compose.all-in-one.yml pull
docker compose -f docker/docker-compose.all-in-one.yml up -d
```

**Multi-service** — default compose builds `docker/Dockerfile` locally. To run the **published** API image, set `ELIDUNE_IMAGE`, pull, and start without building (the compose file defines both `build:` and `image:`; `--no-build` uses the pulled tag):

```bash
export ELIDUNE_IMAGE=ghcr.io/elidune/elidune-server:latest
docker login ghcr.io
docker compose -f docker/docker-compose.yml pull app
docker compose -f docker/docker-compose.yml up -d --no-build
```

Fully local: `docker build -f docker/Dockerfile -t elidune-server:local .` then `docker compose -f docker/docker-compose.yml up -d`.

---

## Configuration with `.env`

Docker Compose automatically reads a file named **`.env`** in the **current working directory** (the directory from which you run `docker compose`, not necessarily the folder that contains the YAML). Variables in `.env` are used for **substitution** in the compose file (`${VAR:-default}`) and are also passed where the compose file wires them into `environment:`.

**Recommended layout**

```
/path/to/elidune-server-rust/    # repo root; run compose from here
├── .env                            # created from docker/.env.example (all-in-one)
└── docker/
    ├── .env.example                # template for docker-compose.all-in-one.yml
    ├── docker-compose.yml
    └── docker-compose.all-in-one.yml
```

For the **all-in-one** stack, start from the template:

```bash
cd /path/to/elidune-server-rust
cp docker/.env.example .env
# edit .env: set ELIDUNE_USERS__JWT_SECRET, ELIDUNE_MEILISEARCH__API_KEY, and optional SMTP
```

Alternatively, keep the file under `docker/` and pass it explicitly (substitution uses this file):

```bash
docker compose --env-file docker/.env.example -f docker/docker-compose.all-in-one.yml config
docker compose --env-file docker/.env -f docker/docker-compose.all-in-one.yml up -d
```

**Application settings (`ELIDUNE_*`)** — The server loads `config/default.toml` inside the image, then **environment variables** override that config. Naming follows the `ELIDUNE_` prefix and nested sections use double underscores, matching `src/config.rs` (e.g. `ELIDUNE_SERVER__PORT`, `ELIDUNE_USERS__JWT_SECRET`, `ELIDUNE_DATABASE__URL`).

**Do not set empty values** for optional settings: empty strings can break deserialization. Omit the variable to use the default from the compose file or TOML.

**All-in-one variables** — See `docker/.env.example` for every variable referenced by `docker-compose.all-in-one.yml`, with comments. At minimum, change **`ELIDUNE_USERS__JWT_SECRET`** and **`ELIDUNE_MEILISEARCH__API_KEY`** (the compose file sets `MEILI_MASTER_KEY` from the same value as the API key).

**Multi-service compose** — The same `ELIDUNE_*` variables apply to the `app` service. Port defaults differ (`API_PORT` defaults to `8080` in `docker-compose.yml`). The `db` service uses fixed dev-style credentials in `docker-compose.yml` (`POSTGRES_USER` / `POSTGRES_PASSWORD`); override those in the YAML and set `ELIDUNE_DATABASE__URL` accordingly for production. The default app URL matches: `postgres://elidune:elidune@db:5432/elidune`.

**Logging** — `RUST_LOG` is read directly by the Rust runtime (not under `ELIDUNE_`), e.g. `RUST_LOG=elidune_server=info,tower_http=info`.

---

## Prerequisites

- **Docker** (20.10+) and **Docker Compose** v2 (`docker compose`).
- **Disk:** plan for several GB (images + PostgreSQL + Meilisearch data).
- **RAM:** 2 GB minimum for light use; more for production load.
- **Ports:** free on the host for the ports you map (see compose files).

```bash
docker --version
docker compose version
```

---

## Deploy: all-in-one (`docker-compose.all-in-one.yml`)

1. Clone or copy the repo and `cd` to the repository root.

2. Copy `docker/.env.example` to `.env` and edit secrets (see [Configuration with `.env`](#configuration-with-env)). Set at least `ELIDUNE_USERS__JWT_SECRET` and a strong `ELIDUNE_MEILISEARCH__API_KEY` (the compose file wires `MEILI_MASTER_KEY` from `ELIDUNE_MEILISEARCH__API_KEY`).

3. Point `ELIDUNE_IMAGE` at `ghcr.io/elidune/elidune-all-in-one:latest` (or a SHA tag) and `docker login ghcr.io` if needed.

4. Start:

```bash
docker compose -f docker/docker-compose.all-in-one.yml pull
docker compose -f docker/docker-compose.all-in-one.yml up -d
```

5. Optional: build the image locally instead of GHCR — requires `docker/Dockerfile.all-in-one` and a longer build:

```bash
docker build -f docker/Dockerfile.all-in-one -t elidune-all-in-one:latest .
# ELIDUNE_IMAGE=elidune-all-in-one:latest in .env or rely on default
docker compose -f docker/docker-compose.all-in-one.yml up -d
```

---

## Deploy: multi-service (`docker-compose.yml`)

1. From the repository root, create `.env` with the variables you need (JWT, database URL if not using defaults, SMTP, etc.).

2. Build and start (default: build API image locally):

```bash
docker compose -f docker/docker-compose.yml build
docker compose -f docker/docker-compose.yml up -d
```

3. Optional **pgAdmin** (profile `tools`):

```bash
docker compose -f docker/docker-compose.yml --profile tools up -d
```

4. To use a pre-built API image instead of `build:`:

```bash
# Set ELIDUNE_IMAGE to your image name and adjust compose if you remove the build section locally
export ELIDUNE_IMAGE=my-registry/elidune-server:1.2.3
```

---

## Verification

**All-in-one**

```bash
docker compose -f docker/docker-compose.all-in-one.yml ps
curl -sS "http://127.0.0.1:${API_PORT:-8282}/api/v1/health"
curl -sS -o /dev/null -w "%{http_code}" "http://127.0.0.1:${GUI_PORT:-8181}/"
```

**Multi-service**

```bash
docker compose -f docker/docker-compose.yml ps
curl -sS "http://127.0.0.1:${API_PORT:-8080}/api/v1/health"
```

**Logs**

```bash
docker compose -f docker/docker-compose.all-in-one.yml logs -f elidune-all-in-one
# or
docker compose -f docker/docker-compose.yml logs -f app
```

---

## Data and volumes

- **All-in-one:** named volumes `elidune-postgres-data`, `elidune-redis-data`, `elidune-meili-data` (see `docker-compose.all-in-one.yml`).
- **Multi-service:** anonymous named volumes `postgres_data`, `meili_data` in the compose file.

Backup examples:

```bash
docker run --rm -v elidune-postgres-data:/data -v "$(pwd):/backup" alpine \
  tar czf /backup/postgres-backup.tgz -C /data .
```

Restore and upgrades should be planned with downtime and tested on a copy first.

---

## Useful commands

```bash
# Stop / start (all-in-one)
docker compose -f docker/docker-compose.all-in-one.yml stop
docker compose -f docker/docker-compose.all-in-one.yml start

# Recreate after .env change
docker compose -f docker/docker-compose.all-in-one.yml up -d

# Shell inside all-in-one container
docker compose -f docker/docker-compose.all-in-one.yml exec elidune-all-in-one sh
```

---

## Troubleshooting

- **Compose cannot find variables:** Run `docker compose` from the directory that contains `.env`, or use `docker compose --env-file /path/to/.env -f docker/docker-compose.all-in-one.yml up -d`.
- **Port already in use:** Change `API_PORT`, `GUI_PORT`, etc. in `.env`, then `docker compose ... down` and `up -d` again.
- **Pull errors for GHCR:** Run `docker login ghcr.io` and ensure you have read access to the package.
- **Empty env vars:** Remove lines with `SOME_VAR=` if the app rejects empty strings.

```bash
docker compose -f docker/docker-compose.all-in-one.yml logs --tail=200 elidune-all-in-one
```

---

## Host Nginx (optional)

To terminate TLS and proxy to the published ports, see [docs/reverse-proxy.md](docs/reverse-proxy.md). Typical upstreams: API on `127.0.0.1:8282` (all-in-one default), GUI on `127.0.0.1:8181`.

---

## Deployment checklist

- [ ] Docker and Compose installed
- [ ] `.env` created (`cp docker/.env.example .env`) or `--env-file docker/.env` used
- [ ] `ELIDUNE_USERS__JWT_SECRET` set to a strong secret
- [ ] `ELIDUNE_MEILISEARCH__API_KEY` set (all-in-one) and consistent with Meilisearch
- [ ] `ELIDUNE_IMAGE` set for all-in-one if using GHCR; `docker login ghcr.io` if private
- [ ] Host ports in `.env` free and firewall-aligned
- [ ] Health endpoint and GUI reachable after `up -d`
- [ ] Backup strategy for volumes

---

## See also

- `docker/.env.example` — commented template for `docker-compose.all-in-one.yml`
- `docker/docker-compose.yml` / `docker/docker-compose.all-in-one.yml` — inline comments for optional `ELIDUNE_*` keys
- `src/config.rs` — mapping of environment variables to configuration
