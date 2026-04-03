# Elidune Complete deployment guide with Docker Compose

This guide explains how to deploy Elidune Complete on a remote server using Docker Compose with persistent volumes.

For TLS and a public hostname in front of **only** the API (or split API + UI), see [docs/reverse-proxy.md](docs/reverse-proxy.md).

## Table of contents

1. [Prerequisites](#prerequisites)
2. [Preparing files](#preparing-files)
3. [Transfer to remote server](#transfer-to-remote-server)
4. [Server configuration](#server-configuration)
5. [Build or import Docker image](#build-or-import-docker-image)
6. [Starting the service](#starting-the-service)
7. [Verification](#verification)
8. [Data management](#data-management)
9. [Useful commands](#useful-commands)
10. [Troubleshooting](#troubleshooting)

---

## Prerequisites

### On the remote server

- **Docker** (20.10 or newer)
- **Docker Compose** (2.0 or newer)
- **Disk space:** at least 5 GB (image + data)
- **RAM:** at least 2 GB recommended
- **Available ports:**
  - 5433 (PostgreSQL) — or as configured
  - 6379 (Redis) — or as configured
  - 8282 (API) — or as configured
  - 8181 (GUI) — or as configured

### Check prerequisites

```bash
# Docker
docker --version

# Docker Compose
docker-compose --version

# Disk space
df -h
```

---

## Preparing files

### Directory layout on the server

```
/opt/elidune/
├── docker-compose.complete.yml
├── .env
├── Dockerfile.complete (optional, for local build)
├── docker/
│   ├── nginx-complete.conf
│   ├── supervisord.conf
│   └── wait-and-start-server.sh
└── scripts/
    ├── dump-db.sh
    ├── import-db.sh
    ├── backup-volumes.sh
    ├── restore-volumes.sh
    └── docker-compose-helper.sh
```

### Files to transfer

#### Required

1. **`docker-compose.complete.yml`** — Docker Compose config
2. **`.env.example`** — Config template (copy to `.env`)
3. **`docker/nginx-complete.conf`** — Internal Nginx config
4. **`docker/supervisord.conf`** — Supervisor config
5. **`docker/wait-and-start-server.sh`** — Server startup script

#### Optional (local build)

6. **`Dockerfile.complete`** — Dockerfile for local image build
7. **`scripts/build-complete-image.sh`** — Build script

#### Recommended utility scripts

8. **`scripts/dump-db.sh`** — Database export
9. **`scripts/import-db.sh`** — Database import
10. **`scripts/backup-volumes.sh`** — Backup Docker volumes
11. **`scripts/restore-volumes.sh`** — Restore Docker volumes
12. **`scripts/docker-compose-helper.sh`** — Common command helper

---

## Transfer to remote server

### Option 1: SCP

```bash
# From your local machine (repository root)
cd /path/to/elidune-server-rust

# Archive required files
tar czf elidune-deploy.tar.gz \
    docker-compose.complete.yml \
    .env.example \
    Dockerfile.complete \
    docker/ \
    scripts/dump-db.sh \
    scripts/import-db.sh \
    scripts/backup-volumes.sh \
    scripts/restore-volumes.sh \
    scripts/docker-compose-helper.sh

# Copy to server
scp elidune-deploy.tar.gz user@remote-server:/opt/elidune/

# On the remote server
ssh user@remote-server
cd /opt/elidune
tar xzf elidune-deploy.tar.gz
```

### Option 2: Git

```bash
# On the remote server
ssh user@remote-server
cd /opt
git clone https://github.com/elidune/elidune-server-rust.git elidune
cd elidune
```

### Option 3: Manual file-by-file

```bash
# Create directories on server
ssh user@remote-server "mkdir -p /opt/elidune/{docker,scripts}"

# Copy files one by one
scp docker-compose.complete.yml user@remote-server:/opt/elidune/
scp .env.example user@remote-server:/opt/elidune/
scp docker/nginx-complete.conf user@remote-server:/opt/elidune/docker/
scp docker/supervisord.conf user@remote-server:/opt/elidune/docker/
scp docker/wait-and-start-server.sh user@remote-server:/opt/elidune/docker/
scp scripts/dump-db.sh user@remote-server:/opt/elidune/scripts/
scp scripts/import-db.sh user@remote-server:/opt/elidune/scripts/
scp scripts/backup-volumes.sh user@remote-server:/opt/elidune/scripts/
scp scripts/restore-volumes.sh user@remote-server:/opt/elidune/scripts/
scp scripts/docker-compose-helper.sh user@remote-server:/opt/elidune/scripts/
```

### Make scripts executable

```bash
# On the remote server
chmod +x /opt/elidune/docker/wait-and-start-server.sh
chmod +x /opt/elidune/scripts/*.sh
```

---

## Server configuration

### 1. Create `.env`

```bash
cd /opt/elidune
cp .env.example .env
nano .env   # or vi .env
```

### 2. Important variables

**⚠️ Change at least these:**

```bash
# Generate a secure JWT secret
openssl rand -base64 32

# Edit .env and set JWT_SECRET
JWT_SECRET=your-generated-key-here

# Change PostgreSQL password (optional but recommended)
POSTGRES_PASSWORD=your-secure-password

# Adjust ports if needed
POSTGRES_PORT=5433
API_PORT=8282
GUI_PORT=8181
REDIS_PORT=6379
```

### 3. Verify configuration

```bash
# Check ports are free
netstat -tuln | grep -E ':(5433|6379|8282|8181)'

# Check Docker
docker ps
```

---

## Build or import Docker image

### Option A: Import a pre-built image (recommended)

If you exported the image from your local machine:

```bash
# On local machine, export image
docker save elidune-complete:latest | gzip > elidune-complete-image.tar.gz

# Copy to server
scp elidune-complete-image.tar.gz user@remote-server:/opt/elidune/

# On server, load image
cd /opt/elidune
gunzip -c elidune-complete-image.tar.gz | docker load
```

### Option B: Build on the server

If you transferred `Dockerfile.complete`:

```bash
cd /opt/elidune

# Build (may take 10–30 minutes)
docker build -f Dockerfile.complete -t elidune-complete:latest .

# Or use build script if present
./scripts/build-complete-image.sh
```

### Confirm image exists

```bash
docker images | grep elidune-complete
```

---

## Starting the service

### 1. Start with Docker Compose

```bash
cd /opt/elidune

# Background
docker-compose -f docker-compose.complete.yml up -d

# Or helper
./scripts/docker-compose-helper.sh start
```

### 2. Check startup

```bash
# Logs
docker-compose -f docker-compose.complete.yml logs -f

# Or helper
./scripts/docker-compose-helper.sh logs

# Status
docker-compose -f docker-compose.complete.yml ps
```

### 3. Wait for services

The container starts PostgreSQL, Redis, then Elidune. Allow 30–60 seconds before testing.

---

## Verification

### 1. Container running

```bash
docker ps | grep elidune-complete
```

### 2. Logs

```bash
# Elidune server
docker-compose -f docker-compose.complete.yml logs elidune-complete | tail -50

# PostgreSQL
docker-compose -f docker-compose.complete.yml exec elidune-complete tail -f /var/log/supervisor/postgresql.out.log

# Rust server
docker-compose -f docker-compose.complete.yml exec elidune-complete tail -f /var/log/supervisor/elidune-server.out.log
```

### 3. Service checks

```bash
# API
curl http://localhost:8282/api/v1/health

# GUI (should return HTML)
curl http://localhost:8181

# PostgreSQL
docker-compose -f docker-compose.complete.yml exec elidune-complete pg_isready -U elidune

# Redis
docker-compose -f docker-compose.complete.yml exec elidune-complete redis-cli ping
```

### 4. Web access

In a browser:
- **GUI:** `http://your-server:8181`
- **API:** `http://your-server:8282/api/v1/health`

---

## Data management

### Export database

```bash
cd /opt/elidune
./scripts/dump-db.sh

# Dump path: /opt/elidune/elidune-db-dump-YYYYMMDD-HHMMSS.sql.gz
```

### Import database

```bash
cd /opt/elidune
./scripts/import-db.sh elidune-db-dump-YYYYMMDD-HHMMSS.sql.gz
```

### Backup Docker volumes

```bash
cd /opt/elidune
./scripts/backup-volumes.sh

# Backups under ./backups/volumes-YYYYMMDD-HHMMSS/
```

### Restore volumes

```bash
cd /opt/elidune
./scripts/restore-volumes.sh ./backups/volumes-YYYYMMDD-HHMMSS
```

---

## Useful commands

### Service control

```bash
# Start
docker-compose -f docker-compose.complete.yml up -d
# or
./scripts/docker-compose-helper.sh start

# Stop
docker-compose -f docker-compose.complete.yml stop
# or
./scripts/docker-compose-helper.sh stop

# Restart
docker-compose -f docker-compose.complete.yml restart
# or
./scripts/docker-compose-helper.sh restart

# Logs
docker-compose -f docker-compose.complete.yml logs -f
# or
./scripts/docker-compose-helper.sh logs

# Status
docker-compose -f docker-compose.complete.yml ps
# or
./scripts/docker-compose-helper.sh status
```

### Container access

```bash
# Shell
docker-compose -f docker-compose.complete.yml exec elidune-complete sh
# or
./scripts/docker-compose-helper.sh shell

# Run psql
docker-compose -f docker-compose.complete.yml exec elidune-complete psql -U elidune -d elidune
```

### Volumes

```bash
# List
docker volume ls | grep elidune

# Inspect
docker volume inspect elidune-postgres-data
docker volume inspect elidune-redis-data

# Disk usage
docker system df -v
```

### Image update

```bash
# Stop
docker-compose -f docker-compose.complete.yml stop

# Load new image
gunzip -c new-image.tar.gz | docker load

# Start
docker-compose -f docker-compose.complete.yml up -d
```

---

## Troubleshooting

### Container won’t start

```bash
docker-compose -f docker-compose.complete.yml logs
docker-compose -f docker-compose.complete.yml ps
```

### PostgreSQL won’t start

```bash
docker-compose -f docker-compose.complete.yml exec elidune-complete \
    tail -f /var/log/supervisor/postgresql.err.log

docker volume inspect elidune-postgres-data
```

### Elidune server won’t start

```bash
docker-compose -f docker-compose.complete.yml exec elidune-complete \
    tail -f /var/log/supervisor/elidune-server.err.log

docker-compose -f docker-compose.complete.yml exec elidune-complete \
    cat /app/config/default.toml
```

### Database migration issues

```bash
docker-compose -f docker-compose.complete.yml exec elidune-complete \
    psql -U elidune -d elidune -c "SELECT * FROM _sqlx_migrations;"

# Reset migrations (⚠️ dangerous)
docker-compose -f docker-compose.complete.yml exec elidune-complete \
    psql -U elidune -d elidune -c "TRUNCATE TABLE _sqlx_migrations;"
```

### Ports already in use

```bash
sudo netstat -tulpn | grep :8181
sudo lsof -i :8181

# Edit .env
nano /opt/elidune/.env
# Change GUI_PORT, API_PORT, etc.

docker-compose -f docker-compose.complete.yml down
docker-compose -f docker-compose.complete.yml up -d
```

### Permissions

```bash
ls -la /opt/elidune/scripts/
chmod +x /opt/elidune/scripts/*.sh
chmod +x /opt/elidune/docker/wait-and-start-server.sh
```

### Clean reset

```bash
# Stop and remove container (keep volumes)
docker-compose -f docker-compose.complete.yml down

# Remove volumes too (⚠️ deletes data)
docker-compose -f docker-compose.complete.yml down -v

docker image prune -a
```

---

## Host Nginx (optional)

To expose Elidune on a domain with HTTPS:

### Example Nginx config

```nginx
server {
    listen 80;
    server_name elidune.example.com;
    return 301 https://$server_name$request_uri;
}

server {
    listen 443 ssl http2;
    server_name elidune.example.com;

    ssl_certificate /etc/letsencrypt/live/example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/example.com/privkey.pem;

    # API
    location /api {
        proxy_pass http://127.0.0.1:8282;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection 'upgrade';
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_set_header X-Forwarded-Host $host;
        proxy_set_header X-Forwarded-Port $server_port;
        proxy_cache_bypass $http_upgrade;
        proxy_connect_timeout 300s;
        proxy_send_timeout 300s;
        proxy_read_timeout 300s;
        proxy_buffering off;
    }

    # GUI
    location / {
        proxy_pass http://127.0.0.1:8181;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection 'upgrade';
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_set_header X-Forwarded-Host $host;
        proxy_set_header X-Forwarded-Port $server_port;
        proxy_cache_bypass $http_upgrade;
    }
}
```

---

## Deployment checklist

- [ ] Docker and Docker Compose installed on server
- [ ] Files copied to server
- [ ] Scripts executable
- [ ] `.env` created and configured
- [ ] `JWT_SECRET` set to a secure value
- [ ] `POSTGRES_PASSWORD` changed (recommended)
- [ ] Ports checked and free
- [ ] Docker image loaded or built
- [ ] Service started with `docker-compose up -d`
- [ ] Logs checked for successful startup
- [ ] API, GUI, PostgreSQL, Redis tested
- [ ] Initial backup done

---

## Support

If something fails, check:

1. Logs: `docker-compose logs -f`
2. Status: `docker-compose ps`
3. Volumes: `docker volume ls`
4. Disk: `df -h`

See also:

- `scripts/README-docker-compose.md` — detailed Docker Compose guide
- Container logs for specific errors
