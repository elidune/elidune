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

# Wait for Meilisearch (same container)
echo "Waiting for Meilisearch to be ready..."
until curl -sf "http://127.0.0.1:7700/health" > /dev/null 2>&1; do
    sleep 1
done
echo "Meilisearch is ready"


# Ensure we're in the correct directory
cd /app



# Start the Elidune server (migrations will run automatically)
echo "Starting Elidune server from $(pwd)..."
exec /app/elidune-server 
