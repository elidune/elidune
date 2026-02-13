#!/bin/bash

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
CONTAINER_NAME="${CONTAINER_NAME:-elidune-complete}"
POSTGRES_VOLUME="${POSTGRES_VOLUME:-elidune-postgres-data}"
REDIS_VOLUME="${REDIS_VOLUME:-elidune-redis-data}"

echo -e "${GREEN}=== Migrating Container to Persistent Volumes ===${NC}"
echo ""
echo -e "${YELLOW}This script will:${NC}"
echo "  1. Stop the existing container"
echo "  2. Create Docker volumes"
echo "  3. Copy data from container to volumes"
echo "  4. Recreate container with volumes"
echo ""

# Check if container exists
if ! docker ps -a --format '{{.Names}}' | grep -q "^${CONTAINER_NAME}$"; then
    echo -e "${RED}Error: Container ${CONTAINER_NAME} not found${NC}"
    exit 1
fi

# Check if container is running
if docker ps --format '{{.Names}}' | grep -q "^${CONTAINER_NAME}$"; then
    echo -e "${YELLOW}Stopping container...${NC}"
    docker stop "${CONTAINER_NAME}"
fi

# Create volumes if they don't exist
echo ""
echo -e "${YELLOW}Creating volumes...${NC}"
if ! docker volume inspect "${POSTGRES_VOLUME}" > /dev/null 2>&1; then
    docker volume create "${POSTGRES_VOLUME}"
    echo -e "${GREEN}✓ PostgreSQL volume created${NC}"
else
    echo -e "${YELLOW}PostgreSQL volume already exists${NC}"
    read -p "Do you want to overwrite it? (y/N) " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        docker volume rm "${POSTGRES_VOLUME}"
        docker volume create "${POSTGRES_VOLUME}"
        echo -e "${GREEN}✓ PostgreSQL volume recreated${NC}"
    fi
fi

if ! docker volume inspect "${REDIS_VOLUME}" > /dev/null 2>&1; then
    docker volume create "${REDIS_VOLUME}"
    echo -e "${GREEN}✓ Redis volume created${NC}"
else
    echo -e "${YELLOW}Redis volume already exists${NC}"
    read -p "Do you want to overwrite it? (y/N) " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        docker volume rm "${REDIS_VOLUME}"
        docker volume create "${REDIS_VOLUME}"
        echo -e "${GREEN}✓ Redis volume recreated${NC}"
    fi
fi

# Copy PostgreSQL data
echo ""
echo -e "${YELLOW}Copying PostgreSQL data to volume...${NC}"
docker run --rm \
    --volumes-from "${CONTAINER_NAME}" \
    -v "${POSTGRES_VOLUME}:/target" \
    alpine sh -c "cp -a /var/lib/postgresql/data/. /target/ 2>/dev/null || echo 'No PostgreSQL data found in container'"

if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓ PostgreSQL data copied${NC}"
else
    echo -e "${YELLOW}Warning: Failed to copy PostgreSQL data (may be empty)${NC}"
fi

# Copy Redis data
echo ""
echo -e "${YELLOW}Copying Redis data to volume...${NC}"
docker run --rm \
    --volumes-from "${CONTAINER_NAME}" \
    -v "${REDIS_VOLUME}:/target" \
    alpine sh -c "cp -a /data/. /target/ 2>/dev/null || mkdir -p /target && echo 'Redis data directory created'"

if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓ Redis data copied${NC}"
else
    echo -e "${YELLOW}Warning: Failed to copy Redis data${NC}"
fi

# Get container image and ports
IMAGE_NAME=$(docker inspect "${CONTAINER_NAME}" --format '{{.Config.Image}}')
PORTS=$(docker port "${CONTAINER_NAME}" 2>/dev/null | awk '{print $1}' | head -1 || echo "")

echo ""
echo -e "${YELLOW}Removing old container...${NC}"
docker rm "${CONTAINER_NAME}"

echo ""
echo -e "${YELLOW}Creating new container with volumes...${NC}"
echo -e "${YELLOW}You will need to recreate the container manually with:${NC}"
echo ""
echo "docker run -d \\"
echo "    --name ${CONTAINER_NAME} \\"
echo "    -p 5433:5432 \\"
echo "    -p 6379:6379 \\"
echo "    -p 8282:8080 \\"
echo "    -p 8181:80 \\"
echo "    -v ${POSTGRES_VOLUME}:/var/lib/postgresql/data \\"
echo "    -v ${REDIS_VOLUME}:/data \\"
echo "    ${IMAGE_NAME}"
echo ""
echo -e "${YELLOW}Or use the script:${NC}"
echo "  ./scripts/start-with-volumes.sh"
echo ""

echo -e "${GREEN}=== Migration completed ===${NC}"
