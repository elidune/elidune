#!/bin/sh
set -e

# Function to handle shutdown
cleanup() {
    echo "Shutting down..."
    kill -TERM "$PG_PID" "$REDIS_PID" "$SERVER_PID" "$NGINX_PID" 2>/dev/null || true
    wait "$PG_PID" "$REDIS_PID" "$SERVER_PID" "$NGINX_PID" 2>/dev/null || true
    exit 0
}

trap cleanup SIGTERM SIGINT

# Start PostgreSQL using the official entrypoint (runs in background)
echo "Starting PostgreSQL..."
docker-entrypoint.sh postgres &
PG_PID=$!

# Wait for PostgreSQL to be ready
echo "Waiting for PostgreSQL to be ready..."
until pg_isready -U elidune -d elidune > /dev/null 2>&1; do
    sleep 1
done
echo "PostgreSQL is ready"

# Start Redis
echo "Starting Redis..."
redis-server --daemonize yes --bind 127.0.0.1 --port 6379
REDIS_PID=$(pgrep -f "redis-server")

# Wait for Redis to be ready
echo "Waiting for Redis to be ready..."
until redis-cli ping > /dev/null 2>&1; do
    sleep 1
done
echo "Redis is ready"

# Set environment variables for Elidune server
export DATABASE_URL="postgres://elidune:elidune@localhost:5432/elidune"
export REDIS_URL="redis://127.0.0.1:6379"
export JWT_SECRET="${JWT_SECRET:-change-this-secret-in-production}"
export RUST_LOG="${RUST_LOG:-elidune_server=info,tower_http=info}"

# Start Elidune server (migrations will run automatically)
echo "Starting Elidune server..."
cd /app
/app/elidune-server &
SERVER_PID=$!

# Wait a bit for the server to start
sleep 2

# Start nginx
echo "Starting nginx..."
nginx -g "daemon off;" &
NGINX_PID=$!

echo "All services started successfully"
echo "  - PostgreSQL: port 5432"
echo "  - Redis: port 6379"
echo "  - Elidune API: port 8080"
echo "  - Web UI: port 80"

# Wait for all processes
wait "$PG_PID" "$REDIS_PID" "$SERVER_PID" "$NGINX_PID"
