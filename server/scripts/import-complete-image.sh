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
CONTAINER_NAME="${CONTAINER_NAME:-elidune-complete}"
IMPORT_DB="${IMPORT_DB:-false}"
DB_FILE=""
POSTGRES_VOLUME="${POSTGRES_VOLUME:-elidune-postgres-data}"
REDIS_VOLUME="${REDIS_VOLUME:-elidune-redis-data}"

# Parse arguments
if [ $# -eq 0 ]; then
    echo -e "${RED}Usage: $0 <image-file.tar.gz> [database-file.sql.gz]${NC}"
    echo -e "${YELLOW}Example: $0 elidune-complete-image-20260212-143653.tar.gz${NC}"
    echo -e "${YELLOW}Example: $0 elidune-complete-image-20260212-143653.tar.gz elidune-database-20260212-143653.sql.gz${NC}"
    exit 1
fi

IMAGE_FILE="$1"
if [ $# -ge 2 ]; then
    DB_FILE="$2"
    IMPORT_DB="true"
fi

# Check if image file exists
if [ ! -f "${IMAGE_FILE}" ]; then
    echo -e "${RED}Error: Image file ${IMAGE_FILE} not found${NC}"
    exit 1
fi

# Check if database file exists (if provided)
if [ "${IMPORT_DB}" = "true" ] && [ ! -f "${DB_FILE}" ]; then
    echo -e "${RED}Error: Database file ${DB_FILE} not found${NC}"
    exit 1
fi

echo -e "${GREEN}=== Importing Complete Elidune Docker Image ===${NC}"
echo -e "${GREEN}(PostgreSQL + Redis + Server + UI)${NC}"
echo ""

# Step 1: Load Docker image
echo -e "${YELLOW}Step 1: Loading Docker image...${NC}"
echo -e "${YELLOW}Loading ${IMAGE_FILE}...${NC}"

gunzip -c "${IMAGE_FILE}" | docker load

if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓ Image imported successfully${NC}"
else
    echo -e "${RED}Error: Failed to import image${NC}"
    exit 1
fi

# Step 1.5: Create volumes if they don't exist
echo ""
echo -e "${YELLOW}Step 1.5: Checking volumes...${NC}"
if ! docker volume inspect "${POSTGRES_VOLUME}" > /dev/null 2>&1; then
    echo -e "${YELLOW}Creating PostgreSQL volume: ${POSTGRES_VOLUME}${NC}"
    docker volume create "${POSTGRES_VOLUME}"
    echo -e "${GREEN}✓ PostgreSQL volume created${NC}"
else
    echo -e "${GREEN}✓ PostgreSQL volume already exists (data will persist)${NC}"
fi

if ! docker volume inspect "${REDIS_VOLUME}" > /dev/null 2>&1; then
    echo -e "${YELLOW}Creating Redis volume: ${REDIS_VOLUME}${NC}"
    docker volume create "${REDIS_VOLUME}"
    echo -e "${GREEN}✓ Redis volume created${NC}"
else
    echo -e "${GREEN}✓ Redis volume already exists (data will persist)${NC}"
fi

# Step 2: Check if container already exists
if docker ps -a --format '{{.Names}}' | grep -q "^${CONTAINER_NAME}$"; then
    echo ""
    echo -e "${YELLOW}Warning: Container ${CONTAINER_NAME} already exists${NC}"
    read -p "Do you want to remove it and create a new one? (y/N) " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        echo -e "${YELLOW}Stopping and removing existing container...${NC}"
        docker stop "${CONTAINER_NAME}" 2>/dev/null || true
        docker rm "${CONTAINER_NAME}" 2>/dev/null || true
    else
        echo -e "${YELLOW}Keeping existing container. Skipping container creation.${NC}"
        echo -e "${YELLOW}You can start it with: docker start ${CONTAINER_NAME}${NC}"
        exit 0
    fi
fi

# Step 3: Run container
echo ""
echo -e "${YELLOW}Step 2: Starting container...${NC}"
echo -e "${YELLOW}Creating and starting container ${CONTAINER_NAME}...${NC}"

docker run -d \
    --name "${CONTAINER_NAME}" \
    -p 5433:5432 \
    -p 6379:6379 \
    -p 8282:8080 \
    -p 8181:80 \
    -v "${POSTGRES_VOLUME}:/var/lib/postgresql/data" \
    -v "${REDIS_VOLUME}:/data" \
    "${IMAGE_NAME}:${IMAGE_TAG}"

if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓ Container started successfully${NC}"
else
    echo -e "${RED}Error: Failed to start container${NC}"
    exit 1
fi

# Wait for PostgreSQL to be ready
echo ""
echo -e "${YELLOW}Waiting for PostgreSQL to be ready...${NC}"
for i in {1..60}; do
    if docker exec "${CONTAINER_NAME}" pg_isready -U elidune -d elidune > /dev/null 2>&1; then
        echo -e "${GREEN}✓ PostgreSQL is ready${NC}"
        break
    fi
    if [ $i -eq 60 ]; then
        echo -e "${RED}Error: PostgreSQL did not become ready in time${NC}"
        exit 1
    fi
    sleep 1
done

# Step 4: Import database (if provided)
if [ "${IMPORT_DB}" = "true" ]; then
    echo ""
    echo -e "${YELLOW}Step 3: Importing database...${NC}"
    echo -e "${YELLOW}Importing ${DB_FILE}...${NC}"
    
    # Wait a bit more for migrations to complete
    sleep 5
    
    gunzip -c "${DB_FILE}" | docker exec -i "${CONTAINER_NAME}" psql -U elidune -d elidune > /dev/null
    
    if [ $? -eq 0 ]; then
        echo -e "${GREEN}✓ Database imported successfully${NC}"
    else
        echo -e "${RED}Error: Failed to import database${NC}"
        echo -e "${YELLOW}You can try importing manually:${NC}"
        echo "  gunzip -c ${DB_FILE} | docker exec -i ${CONTAINER_NAME} psql -U elidune -d elidune"
    fi
fi

# Summary
echo ""
echo -e "${GREEN}=== Import completed successfully ===${NC}"
echo ""
echo -e "${YELLOW}Container status:${NC}"
docker ps --filter "name=${CONTAINER_NAME}" --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}"
echo ""
echo -e "${YELLOW}Volumes (data persists):${NC}"
echo "  - PostgreSQL: ${POSTGRES_VOLUME}"
echo "  - Redis: ${REDIS_VOLUME}"
echo ""
echo -e "${YELLOW}To access:${NC}"
echo "  - Web UI: http://localhost:8181"
echo "  - API: http://localhost:8282"
echo "  - PostgreSQL: localhost:5433"
echo "  - Redis: localhost:6379"
echo ""
echo -e "${YELLOW}Useful commands:${NC}"
echo "  # View logs:"
echo "    docker logs -f ${CONTAINER_NAME}"
echo ""
echo "  # Stop container (data persists in volumes):"
echo "    docker stop ${CONTAINER_NAME}"
echo ""
echo "  # Start container (data will be restored from volumes):"
echo "    docker start ${CONTAINER_NAME}"
echo ""
echo "  # Remove container (volumes persist, data is safe):"
echo "    docker rm -f ${CONTAINER_NAME}"
echo ""
echo "  # Backup volumes:"
echo "    ./scripts/backup-volumes.sh"
echo ""
echo -e "${YELLOW}Default credentials:${NC}"
echo "  PostgreSQL user: elidune"
echo "  PostgreSQL password: elidune"
echo "  PostgreSQL database: elidune"
echo ""
echo -e "${RED}⚠️  Important: Change the PostgreSQL password in production!${NC}"
