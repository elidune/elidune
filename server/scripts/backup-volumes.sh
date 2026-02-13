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
TIMESTAMP=$(date +%Y%m%d-%H%M%S)
BACKUP_DIR="${BACKUP_DIR:-./backups/volumes-${TIMESTAMP}}"
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

echo -e "${GREEN}=== Backing up Docker Volumes ===${NC}"
echo ""

# Create backup directory
mkdir -p "${BACKUP_DIR}"

# Backup PostgreSQL volume
if docker volume inspect "${POSTGRES_VOLUME}" > /dev/null 2>&1; then
    echo -e "${YELLOW}Backing up PostgreSQL volume: ${POSTGRES_VOLUME}...${NC}"
    
    # Create a temporary container to backup the volume
    docker run --rm \
        -v "${POSTGRES_VOLUME}:/source:ro" \
        -v "${PROJECT_ROOT}/${BACKUP_DIR}:/backup" \
        alpine tar czf /backup/postgres-data-${TIMESTAMP}.tar.gz -C /source .
    
    if [ $? -eq 0 ]; then
        FILE_SIZE=$(du -h "${BACKUP_DIR}/postgres-data-${TIMESTAMP}.tar.gz" | cut -f1)
        echo -e "${GREEN}✓ PostgreSQL volume backed up (${FILE_SIZE})${NC}"
    else
        echo -e "${RED}Error: Failed to backup PostgreSQL volume${NC}"
        exit 1
    fi
else
    echo -e "${YELLOW}PostgreSQL volume ${POSTGRES_VOLUME} does not exist, skipping...${NC}"
fi

# Backup Redis volume
if docker volume inspect "${REDIS_VOLUME}" > /dev/null 2>&1; then
    echo -e "${YELLOW}Backing up Redis volume: ${REDIS_VOLUME}...${NC}"
    
    docker run --rm \
        -v "${REDIS_VOLUME}:/source:ro" \
        -v "${PROJECT_ROOT}/${BACKUP_DIR}:/backup" \
        alpine tar czf /backup/redis-data-${TIMESTAMP}.tar.gz -C /source .
    
    if [ $? -eq 0 ]; then
        FILE_SIZE=$(du -h "${BACKUP_DIR}/redis-data-${TIMESTAMP}.tar.gz" | cut -f1)
        echo -e "${GREEN}✓ Redis volume backed up (${FILE_SIZE})${NC}"
    else
        echo -e "${RED}Error: Failed to backup Redis volume${NC}"
        exit 1
    fi
else
    echo -e "${YELLOW}Redis volume ${REDIS_VOLUME} does not exist, skipping...${NC}"
fi

echo ""
echo -e "${GREEN}=== Backup completed ===${NC}"
echo -e "${GREEN}Backup directory: ${BACKUP_DIR}${NC}"
echo ""
echo -e "${YELLOW}To restore volumes:${NC}"
echo "  ./scripts/restore-volumes.sh ${BACKUP_DIR}"
echo ""
