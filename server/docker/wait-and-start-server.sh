#!/bin/sh
set -e

# Wait for PostgreSQL to be ready
echo "Waiting for PostgreSQL to be ready..."
until pg_isready -U elidune -d elidune > /dev/null 2>&1; do
    sleep 1
done
echo "PostgreSQL is ready"

# Wait for Redis to be ready
echo "Waiting for Redis to be ready..."
until redis-cli ping > /dev/null 2>&1; do
    sleep 1
done
echo "Redis is ready"

# Set environment variables
export DATABASE_URL="${DATABASE_URL:-postgres://elidune:elidune@localhost:5432/elidune}"
export REDIS_URL="${REDIS_URL:-redis://127.0.0.1:6379}"
export JWT_SECRET="${JWT_SECRET:-change-this-secret-in-production}"
export RUST_LOG="${RUST_LOG:-elidune_server=info,tower_http=info}"

# Ensure we're in the correct directory
cd /app

# Verify config file exists
if [ ! -f /app/config/default.toml ]; then
    echo "ERROR: /app/config/default.toml not found!"
    echo "Creating default configuration..."
    mkdir -p /app/config
    cat > /app/config/default.toml << 'CONFIG_EOF'
# Elidune Server Configuration

[server]
host = "0.0.0.0"
port = 8080

[database]
url = "postgres://elidune:elidune@localhost:5432/elidune"
max_connections = 10
min_connections = 2

[users]
jwt_secret = "change-this-secret-in-production"
jwt_expiration_hours = 24

[logging]
level = "info"
format = "pretty"

[email]
smtp_host = ""
smtp_port = 587
smtp_username = ""
smtp_password = ""
smtp_from = ""
smtp_from_name = "Elidune"
smtp_use_tls = true

[redis]
url = "redis://127.0.0.1:6379"
z3950_cache_ttl_seconds = 604800
CONFIG_EOF
fi

# Start the Elidune server (migrations will run automatically)
echo "Starting Elidune server from $(pwd)..."
echo "Config file: $(ls -la /app/config/default.toml)"
exec /app/elidune-server
