#!/bin/bash

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
DB_HOST="${DB_HOST:-localhost}"
DB_PORT="${DB_PORT:-5432}"
DB_USER="${DB_USER:-elidune}"
DB_PASSWORD="${DB_PASSWORD:-elidune}"
DB_NAME="${DB_NAME:-elidune}"
IMAGE_NAME="${IMAGE_NAME:-elidune-complete}"
IMAGE_TAG="${IMAGE_TAG:-latest}"
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DUMP_FILE="${PROJECT_ROOT}/scripts/elidune-pgdump-$(date +%Y%m%d-%H%M%S).sql"
TEMP_DIR=$(mktemp -d)

echo -e "${GREEN}=== Building complete Elidune Docker image (PostgreSQL + Server) ===${NC}"

# Ensure scripts directory exists
mkdir -p "${PROJECT_ROOT}/scripts"

# Step 1: Export database from localhost
echo -e "${YELLOW}Step 1: Exporting database from ${DB_HOST}:${DB_PORT}...${NC}"
export PGPASSWORD="${DB_PASSWORD}"
pg_dump -h "${DB_HOST}" -p "${DB_PORT}" -U "${DB_USER}" -d "${DB_NAME}" \
    --clean --if-exists --no-owner --no-acl \
    --format=plain > "${DUMP_FILE}"

if [ ! -s "${DUMP_FILE}" ]; then
    echo -e "${RED}Error: Dump file is empty or not created${NC}"
    exit 1
fi

echo -e "${GREEN}✓ Database exported to ${DUMP_FILE}${NC}"

# Step 2: Create Dockerfile for complete image
echo -e "${YELLOW}Step 2: Creating Dockerfile for complete Elidune image...${NC}"

cat > "${TEMP_DIR}/Dockerfile" <<'DOCKERFILE_EOF'
# Build stage for Elidune server
FROM rust:1.90-bookworm AS builder

WORKDIR /app

# Install dependencies for building
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy manifests
COPY Cargo.toml Cargo.lock ./

# Create dummy main.rs to build dependencies
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src

# Copy source code
COPY src ./src
COPY migrations ./migrations
COPY config ./config

# Build the application
RUN touch src/main.rs && \
    cargo build --release

# Runtime stage: PostgreSQL + Elidune server
FROM postgres:16

# Install runtime dependencies for Elidune server
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Set PostgreSQL environment variables
ENV POSTGRES_USER=elidune
ENV POSTGRES_PASSWORD=elidune
ENV POSTGRES_DB=elidune

# Copy database dump to init directory
COPY elidune-dump.sql /docker-entrypoint-initdb.d/01-elidune-dump.sql

# Copy Elidune server binary and files
COPY --from=builder /app/target/release/elidune-server /app/elidune-server
COPY --from=builder /app/migrations /app/migrations
COPY --from=builder /app/config /app/config

# Make binary executable
RUN chmod +x /app/elidune-server

# Create startup script that runs both PostgreSQL and Elidune server
RUN cat > /app/start.sh <<'SCRIPT_EOF'
#!/bin/sh
set -e

# Function to handle shutdown
cleanup() {
    echo "Shutting down..."
    kill -TERM "$PG_PID" "$SERVER_PID" 2>/dev/null || true
    wait "$PG_PID" "$SERVER_PID" 2>/dev/null || true
    exit 0
}

trap cleanup SIGTERM SIGINT

# Start PostgreSQL using the official entrypoint (runs in background)
docker-entrypoint.sh postgres &
PG_PID=$!

# Wait for PostgreSQL to be ready
echo "Waiting for PostgreSQL to be ready..."
until pg_isready -U elidune -d elidune > /dev/null 2>&1; do
    sleep 1
done
echo "PostgreSQL is ready"

# Set environment variables for Elidune server
export DATABASE_URL="postgres://elidune:elidune@localhost:5432/elidune"
export JWT_SECRET="${JWT_SECRET:-change-this-secret-in-production}"
export RUST_LOG="${RUST_LOG:-elidune_server=info,tower_http=info}"

# Start Elidune server
echo "Starting Elidune server..."
cd /app
/app/elidune-server &
SERVER_PID=$!

# Wait for both processes
wait "$PG_PID" "$SERVER_PID"
SCRIPT_EOF

RUN chmod +x /app/start.sh

# Expose ports
EXPOSE 5432 8080

# Use the startup script
CMD ["/app/start.sh"]
DOCKERFILE_EOF

# Copy necessary files to temp directory
echo -e "${YELLOW}Step 3: Copying project files...${NC}"
cp "${DUMP_FILE}" "${TEMP_DIR}/elidune-dump.sql"
cp "${PROJECT_ROOT}/Cargo.toml" "${TEMP_DIR}/"
if [ -f "${PROJECT_ROOT}/Cargo.lock" ]; then
    cp "${PROJECT_ROOT}/Cargo.lock" "${TEMP_DIR}/"
fi
cp -r "${PROJECT_ROOT}/src" "${TEMP_DIR}/"
cp -r "${PROJECT_ROOT}/migrations" "${TEMP_DIR}/"
cp -r "${PROJECT_ROOT}/config" "${TEMP_DIR}/"

echo -e "${GREEN}✓ Files copied${NC}"

# Step 4: Build Docker image
echo -e "${YELLOW}Step 4: Building Docker image ${IMAGE_NAME}:${IMAGE_TAG}...${NC}"
echo -e "${YELLOW}(This may take several minutes for the Rust build)...${NC}"
cd "${TEMP_DIR}"
docker build -t "${IMAGE_NAME}:${IMAGE_TAG}" .

if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓ Docker image built successfully${NC}"
    echo -e "${GREEN}Image: ${IMAGE_NAME}:${IMAGE_TAG}${NC}"
    echo ""
    echo -e "${YELLOW}To run the container:${NC}"
    echo "  docker run -d --name elidune-complete -p 5432:5432 -p 8080:8080 ${IMAGE_NAME}:${IMAGE_TAG}"
    echo ""
    echo -e "${YELLOW}With custom environment variables:${NC}"
    echo "  docker run -d --name elidune-complete \\"
    echo "    -p 5432:5432 -p 8080:8080 \\"
    echo "    -e JWT_SECRET=your-secret-key \\"
    echo "    -e RUST_LOG=elidune_server=debug \\"
    echo "    ${IMAGE_NAME}:${IMAGE_TAG}"
    echo ""
    echo -e "${YELLOW}To access:${NC}"
    echo "  - API: http://localhost:8080"
    echo "  - PostgreSQL: localhost:5432"
else
    echo -e "${RED}Error: Failed to build Docker image${NC}"
    exit 1
fi

# Cleanup
cd - > /dev/null
rm -rf "${TEMP_DIR}"

echo -e "${GREEN}=== Done ===${NC}"
