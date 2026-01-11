# Elidune Server (Rust)

Modern library management system, rewritten in Rust with a JSON REST API.

## Description

Elidune is a library management server that allows you to:

- Manage a document catalog (books, CDs, DVDs, periodicals, etc.)
- Manage patrons and their subscriptions
- Manage loans and returns
- Import records from remote catalogs via Z39.50
- Generate statistics

This version is a complete rewrite of the original C server, with the following improvements:

- Modern JSON REST API (instead of XML-RPC)
- Secure JWT authentication
- Password hashing with Argon2
- PostgreSQL full-text search
- Automatic OpenAPI/Swagger documentation
- Improved performance with async/await

## Prerequisites

- Rust 1.75+
- PostgreSQL 14+
- Docker (optional)

## Database Initialization

### Option 1: With Docker (recommended)

Docker Compose automatically creates the database. No additional action is required.

```bash
docker compose up -d
```

### Option 2: Existing PostgreSQL in Docker

If you already have a PostgreSQL container running:

```bash
# 1. Identify the name or ID of your PostgreSQL container
docker ps | grep postgres

# 2. Connect to the container and create the database
docker exec -it <postgres_container_name> psql -U postgres

# In the psql shell:
CREATE USER elidune WITH PASSWORD 'elidune';
CREATE DATABASE elidune OWNER elidune;
GRANT ALL PRIVILEGES ON DATABASE elidune TO elidune;

# For PostgreSQL 15+
\c elidune
GRANT ALL ON SCHEMA public TO elidune;
\q
```

Or in a single command:

```bash
# Replace <container_name> with your PostgreSQL container name
docker exec -it <container_name> psql -U postgres -c "CREATE USER elidune WITH PASSWORD 'elidune';"
docker exec -it <container_name> psql -U postgres -c "CREATE DATABASE elidune OWNER elidune;"
docker exec -it <container_name> psql -U postgres -c "GRANT ALL PRIVILEGES ON DATABASE elidune TO elidune;"
```

Then run the migrations from your machine:

```bash
# Install sqlx-cli
cargo install sqlx-cli --no-default-features --features postgres

# Configure the URL (adjust the port if different from 5432)
export DATABASE_URL="postgres://elidune:elidune@localhost:5432/elidune"

# Run migrations
cd /home/cjean/Documents/Developments/elidune/elidune-server-rust
sqlx migrate run
```

Verify that the tables are created:

```bash
docker exec -it <container_name> psql -U elidune -d elidune -c "\dt"
```

### Option 3: Manual PostgreSQL Installation

#### 1. Install PostgreSQL

```bash
# Ubuntu/Debian
sudo apt update
sudo apt install postgresql postgresql-contrib

# Fedora/RHEL
sudo dnf install postgresql-server postgresql-contrib
sudo postgresql-setup --initdb
sudo systemctl start postgresql

# macOS with Homebrew
brew install postgresql@16
brew services start postgresql@16
```

#### 2. Create the user and database

```bash
# Connect as postgres
sudo -u postgres psql

# In the psql shell, run:
CREATE USER elidune WITH PASSWORD 'elidune';
CREATE DATABASE elidune OWNER elidune;
GRANT ALL PRIVILEGES ON DATABASE elidune TO elidune;

# For PostgreSQL 15+, also grant privileges on the public schema
\c elidune
GRANT ALL ON SCHEMA public TO elidune;

# Exit
\q
```

#### 3. Configure access (if necessary)

Edit `/etc/postgresql/16/main/pg_hba.conf` (path may vary):

```
# Add this line to allow local connections with password
host    elidune         elidune         127.0.0.1/32            scram-sha-256
```

Restart PostgreSQL:

```bash
sudo systemctl restart postgresql
```

#### 4. Run migrations

```bash
# Install sqlx-cli if not already done
cargo install sqlx-cli --no-default-features --features postgres

# Configure the connection URL
export DATABASE_URL="postgres://elidune:elidune@localhost:5432/elidune"

# Run migrations
sqlx migrate run
```

### Verify Installation

```bash
# Test the connection
psql -h localhost -U elidune -d elidune -c "SELECT version();"

# Verify the created tables
psql -h localhost -U elidune -d elidune -c "\dt"
```

### Default User

After migrations, an administrator user is created:

| Field | Value |
|-------|-------|
| Login | `admin` |
| Password | `admin` |
| Account type | Administrator |

⚠️ **Important**: Change this password in production!

## Installation

### With Docker

```bash
# Clone the project
git clone https://github.com/elidune/elidune-server-rust.git
cd elidune-server-rust

# Start with Docker Compose
docker compose up -d

# The API is available at http://localhost:8080
```

### Without Docker

```bash
# Install dependencies
cargo build --release

# Configure the database
export DATABASE_URL="postgres://elidune:elidune@localhost:5432/elidune"

# Run migrations
cargo sqlx migrate run

# Start the server
./target/release/elidune-server
```

## Configuration

The server can be configured via:

1. Configuration files (`config/default.toml`)
2. Environment variables (prefix `ELIDUNE_`)
3. `.env` file

### Main Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `DATABASE_URL` | PostgreSQL connection URL | `postgres://elidune:elidune@localhost:5432/elidune` |
| `JWT_SECRET` | Secret for signing JWT tokens | (must be changed in production) |
| `SERVER_HOST` | Listen address | `0.0.0.0` |
| `SERVER_PORT` | Listen port | `8080` |
| `RUST_LOG` | Log level | `info` |

## API

### Documentation

OpenAPI documentation is available at:

- Swagger UI: `http://localhost:8080/swagger-ui`
- OpenAPI JSON: `http://localhost:8080/api-docs/openapi.json`

### Main Endpoints

#### Authentication

```bash
# Login
curl -X POST http://localhost:8080/api/v1/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username": "admin", "password": "admin"}'

# User profile
curl http://localhost:8080/api/v1/auth/me \
  -H "Authorization: Bearer <token>"
```

#### Catalog

```bash
# Search documents
curl "http://localhost:8080/api/v1/items?title=tolkien" \
  -H "Authorization: Bearer <token>"

# Document details
curl http://localhost:8080/api/v1/items/1 \
  -H "Authorization: Bearer <token>"

# Create a document
curl -X POST http://localhost:8080/api/v1/items \
  -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json" \
  -d '{"title1": "The Lord of the Rings", "media_type": "b"}'
```

#### Patrons

```bash
# List patrons
curl http://localhost:8080/api/v1/users \
  -H "Authorization: Bearer <token>"

# Create a patron
curl -X POST http://localhost:8080/api/v1/users \
  -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json" \
  -d '{"login": "john", "password": "secret", "firstname": "John", "lastname": "Doe"}'
```

#### Loans

```bash
# Borrow a document
curl -X POST http://localhost:8080/api/v1/loans \
  -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json" \
  -d '{"user_id": 2, "specimen_identification": "ABC123"}'

# Return a document
curl -X POST http://localhost:8080/api/v1/loans/1/return \
  -H "Authorization: Bearer <token>"
```

## Migration from Old Version

A Python script is provided to migrate data:

```bash
python scripts/migrate_data.py \
  --source-db "postgres://old_user:old_pass@old_host/old_db" \
  --target-db "postgres://elidune:elidune@localhost/elidune"
```

### Migration Test with Sample Data

A SQL file with test data is provided to test the migration:

```bash
# 1. Create a "legacy" database simulating the old format
docker exec -it <postgres_container_name> psql -U postgres -c "CREATE DATABASE elidune_legacy OWNER elidune;"

# 2. Import test data
docker exec -i <postgres_container_name> psql -U elidune -d elidune_legacy < scripts/sample_legacy_data.sql

# 3. Run the migration
python scripts/migrate_data.py \
  --source-db "postgres://elidune:elidune@localhost/elidune_legacy" \
  --target-db "postgres://elidune:elidune@localhost/elidune"
```

The `sample_legacy_data.sql` file contains:
- 7 patrons (admin, librarian, readers, guest)
- 10 authors (Hugo, Tolkien, Rowling, Goscinny, etc.)
- 14 documents (books, comics, CDs, DVDs)
- 20 specimens
- 5 current loans + 5 archived
- Z39.50 server configuration (BnF, SUDOC)

Test accounts:
| Login | Password | Role |
|-------|----------|------|
| admin | admin | Administrator |
| biblio | biblio123 | Librarian |
| lecteur1 | pass123 | Reader |

## Development

```bash
# Run in development mode
cargo run

# Run tests
cargo test

# Run integration tests (requires a running server)
cargo test -- --ignored

# Check formatting
cargo fmt --check

# Linter
cargo clippy
```

## Architecture

```
src/
├── api/          # HTTP handlers (REST endpoints)
├── models/       # Data structures
├── repository/   # Database access
├── services/     # Business logic
├── marc/         # MARC parsing and translation
├── config.rs     # Configuration
├── error.rs      # Error handling
└── main.rs       # Entry point
```

## License

GPL-2.0 - See [COPYING](COPYING)

## Authors

- Jean Collonville <cjean@elidune.org>

## History

- **v0.6.0** - Rewrite in Rust with JSON REST API
- **v0.5.1** - Last C version with XML-RPC
