#!/bin/bash

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
POSTGRES_VOLUME="${POSTGRES_VOLUME:-elidune-postgres-data}"
REDIS_VOLUME="${REDIS_VOLUME:-elidune-redis-data}"
CONTAINER_NAME="${CONTAINER_NAME:-elidune-complete}"

if [ $# -eq 0 ]; then
    echo -e "${RED}Usage: $0 <backup-directory>${NC}"
    echo -e "${YELLOW}Example: $0 ./backups/volumes-20260212-143653${NC}"
    exit 1
fi

BACKUP_DIR="$1"

if [ ! -d "${BACKUP_DIR}" ]; then
    echo -e "${RED}Error: Backup directory ${BACKUP_DIR} not found${NC}"
    exit 1
fi

echo -e "${GREEN}=== Restoring Docker Volumes ===${NC}"
echo ""
echo -e "${YELLOW}⚠️  WARNING: This will replace existing volume data!${NC}"
echo -e "${YELLOW}Make sure the container ${CONTAINER_NAME} is stopped${NC}"
echo ""
read -p "Do you want to continue? (y/N) " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo -e "${YELLOW}Cancelled${NC}"
    exit 0
fi

# Stop container if running
if docker ps --format '{{.Names}}' | grep -q "^${CONTAINER_NAME}$"; then
    echo -e "${YELLOW}Stopping container...${NC}"
    docker stop "${CONTAINER_NAME}"
fi

# Remove existing volumes
echo ""
echo -e "${YELLOW}Removing existing volumes...${NC}"
docker volume rm "${POSTGRES_VOLUME}" 2>/dev/null || true
docker volume rm "${REDIS_VOLUME}" 2>/dev/null || true

# Create new volumes
echo -e "${YELLOW}Creating new volumes...${NC}"
docker volume create "${POSTGRES_VOLUME}"
docker volume create "${REDIS_VOLUME}"

# Restore PostgreSQL volume
POSTGRES_BACKUP=$(find "${BACKUP_DIR}" -name "postgres-data-*.tar.gz" | head -1)
if [ -n "${POSTGRES_BACKUP}" ]; then
    echo ""
    echo -e "${YELLOW}Restoring PostgreSQL volume from ${POSTGRES_BACKUP}...${NC}"
    docker run --rm \
        -v "${POSTGRES_VOLUME}:/target" \
        -v "$(pwd)/${BACKUP_DIR}:/backup:ro" \
        alpine sh -c "cd /target && tar xzf /backup/$(basename ${POSTGRES_BACKUP})"
    
    if [ $? -eq 0 ]; then
        echo -e "${GREEN}✓ PostgreSQL volume restored${NC}"
    else
        echo -e "${RED}Error: Failed to restore PostgreSQL volume${NC}"
        exit 1
    fi
else
    echo -e "${YELLOW}No PostgreSQL backup found, skipping...${NC}"
fi

# Restore Redis volume
REDIS_BACKUP=$(find "${BACKUP_DIR}" -name "redis-data-*.tar.gz" | head -1)
if [ -n "${REDIS_BACKUP}" ]; then
    echo ""
    echo -e "${YELLOW}Restoring Redis volume from ${REDIS_BACKUP}...${NC}"
    docker run --rm \
        -v "${REDIS_VOLUME}:/target" \
        -v "$(pwd)/${BACKUP_DIR}:/backup:ro" \
        alpine sh -c "cd /target && tar xzf /backup/$(basename ${REDIS_BACKUP})"
    
    if [ $? -eq 0 ]; then
        echo -e "${GREEN}✓ Redis volume restored${NC}"
    else
        echo -e "${RED}Error: Failed to restore Redis volume${NC}"
        exit 1
    fi
else
    echo -e "${YELLOW}No Redis backup found, skipping...${NC}"
fi

echo ""
echo -e "${GREEN}=== Restore completed ===${NC}"
echo ""
echo -e "${YELLOW}Next steps:${NC}"
echo "  1. Start the container: docker start ${CONTAINER_NAME}"
echo "  2. Or create a new container: ./scripts/start-with-volumes.sh"
echo ""
