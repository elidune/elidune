#!/bin/bash

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
IMAGE_NAME="${IMAGE_NAME:-elidune-complete}"
IMAGE_TAG="${IMAGE_TAG:-latest}"
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

echo -e "${GREEN}=== Building complete Elidune Docker image ===${NC}"
echo -e "${GREEN}(PostgreSQL + Redis + Server + UI)${NC}"
echo ""

# Ensure docker directory exists
mkdir -p "${PROJECT_ROOT}/docker"

# Check if required files exist
if [ ! -f "${PROJECT_ROOT}/docker/nginx-complete.conf" ]; then
    echo -e "${RED}Error: nginx-complete.conf not found${NC}"
    exit 1
fi

if [ ! -f "${PROJECT_ROOT}/docker/supervisord.conf" ]; then
    echo -e "${RED}Error: supervisord.conf not found${NC}"
    exit 1
fi

if [ ! -f "${PROJECT_ROOT}/docker/wait-and-start-server.sh" ]; then
    echo -e "${RED}Error: wait-and-start-server.sh not found${NC}"
    exit 1
fi

if [ ! -f "${PROJECT_ROOT}/Dockerfile.complete" ]; then
    echo -e "${RED}Error: Dockerfile.complete not found${NC}"
    exit 1
fi

# Build Docker image
echo -e "${YELLOW}Building Docker image ${IMAGE_NAME}:${IMAGE_TAG}...${NC}"
echo -e "${YELLOW}(This may take several minutes for the Rust and Node.js builds)...${NC}"
cd "${PROJECT_ROOT}"
docker build --no-cache -f Dockerfile.complete -t "${IMAGE_NAME}:${IMAGE_TAG}" .

if [ $? -eq 0 ]; then
    echo ""
    echo -e "${GREEN}✓ Docker image built successfully${NC}"
    echo -e "${GREEN}Image: ${IMAGE_NAME}:${IMAGE_TAG}${NC}"
    echo ""
    echo -e "${YELLOW}To run the container with persistent volumes (recommended):${NC}"
    echo "  ./scripts/start-with-volumes.sh"
    echo ""
    echo -e "${YELLOW}Or manually with volumes:${NC}"
    echo '  docker run -d --name elidune-complete \'
    echo '    -p 5433:5432 \'
    echo '    -p 6379:6379 \'
    echo '    -p 8282:8080 \'
    echo '    -p 8181:80 \'
    echo '    -v elidune-postgres-data:/var/lib/postgresql/data \'
    echo '    -v elidune-redis-data:/data \'
    echo "    ${IMAGE_NAME}:${IMAGE_TAG}"
    echo ""
    echo -e "${YELLOW}With custom environment variables:${NC}"
    echo '  docker run -d --name elidune-complete \'
    echo '    -p 5433:5432 -p 6379:6379 -p 8282:8080 -p 8181:80 \'
    echo '    -v elidune-postgres-data:/var/lib/postgresql/data \'
    echo '    -v elidune-redis-data:/data \'
    echo "    ${IMAGE_NAME}:${IMAGE_TAG}"
    echo ""
    echo -e "${YELLOW}To access:${NC}"
    echo "  - Web UI: http://localhost:8181"
    echo "  - API: http://localhost:8282"
    echo "  - PostgreSQL: localhost:5433"
    echo "  - Redis: localhost:6379"
    echo ""
    echo -e "${YELLOW}Note:${NC}"
    echo "  The database will be initialized automatically with migrations."
    echo "  Data is stored in Docker volumes and will persist across container restarts."
    echo "  Use ./scripts/backup-volumes.sh to backup your data."
else
    echo -e "${RED}Error: Failed to build Docker image${NC}"
    exit 1
fi

echo -e "${GREEN}=== Done ===${NC}"
