#!/bin/bash

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
CONTAINER_NAME="${CONTAINER_NAME:-elidune-complete}"
IMAGE_NAME="${IMAGE_NAME:-elidune-complete}"
IMAGE_TAG="${IMAGE_TAG:-latest}"
POSTGRES_VOLUME="${POSTGRES_VOLUME:-elidune-postgres-data}"
REDIS_VOLUME="${REDIS_VOLUME:-elidune-redis-data}"

echo -e "${GREEN}=== Starting Elidune Container with Persistent Volumes ===${NC}"
echo ""

# Check if image exists
if ! docker image inspect "${IMAGE_NAME}:${IMAGE_TAG}" > /dev/null 2>&1; then
    echo -e "${RED}Error: Image ${IMAGE_NAME}:${IMAGE_TAG} not found${NC}"
    echo -e "${YELLOW}Please import or build the image first${NC}"
    exit 1
fi

# Create volumes if they don't exist
echo -e "${YELLOW}Checking volumes...${NC}"
if ! docker volume inspect "${POSTGRES_VOLUME}" > /dev/null 2>&1; then
    echo -e "${YELLOW}Creating PostgreSQL volume: ${POSTGRES_VOLUME}${NC}"
    docker volume create "${POSTGRES_VOLUME}"
    echo -e "${GREEN}✓ PostgreSQL volume created${NC}"
else
    echo -e "${GREEN}✓ PostgreSQL volume already exists${NC}"
fi

if ! docker volume inspect "${REDIS_VOLUME}" > /dev/null 2>&1; then
    echo -e "${YELLOW}Creating Redis volume: ${REDIS_VOLUME}${NC}"
    docker volume create "${REDIS_VOLUME}"
    echo -e "${GREEN}✓ Redis volume created${NC}"
else
    echo -e "${GREEN}✓ Redis volume already exists${NC}"
fi

# Check if container already exists
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
        echo -e "${YELLOW}Keeping existing container. Starting it...${NC}"
        docker start "${CONTAINER_NAME}"
        exit 0
    fi
fi

# Run container with volumes
echo ""
echo -e "${YELLOW}Starting container with persistent volumes...${NC}"

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

echo ""
echo -e "${GREEN}=== Container started successfully ===${NC}"
echo ""
echo -e "${YELLOW}Volumes:${NC}"
echo "  - PostgreSQL: ${POSTGRES_VOLUME} -> /var/lib/postgresql/data"
echo "  - Redis: ${REDIS_VOLUME} -> /data"
echo ""
echo -e "${YELLOW}Ports:${NC}"
echo "  - PostgreSQL: localhost:5433 -> container:5432"
echo "  - Redis: localhost:6379 -> container:6379"
echo "  - API: localhost:8282 -> container:8080"
echo "  - Web UI: localhost:8181 -> container:80"
echo ""
echo -e "${YELLOW}Useful commands:${NC}"
echo "  # View logs:"
echo "    docker logs -f ${CONTAINER_NAME}"
echo ""
echo "  # Stop container:"
echo "    docker stop ${CONTAINER_NAME}"
echo ""
echo "  # Start container (data will persist):"
echo "    docker start ${CONTAINER_NAME}"
echo ""
echo "  # Remove container (volumes will persist):"
echo "    docker rm -f ${CONTAINER_NAME}"
echo ""
echo "  # Backup volumes:"
echo "    ./scripts/backup-volumes.sh"
echo ""
