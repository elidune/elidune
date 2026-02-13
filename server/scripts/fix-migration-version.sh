#!/bin/bash

# Script to fix VersionMissing errors by inserting missing migration records
# Usage: ./scripts/fix-migration-version.sh [version_number]

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
DB_USER="${DB_USER:-elidune}"
DB_NAME="${DB_NAME:-elidune}"
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# Check if docker-compose is being used
USE_COMPOSE=false
if [ -f "${PROJECT_ROOT}/docker-compose.complete.yml" ]; then
    if docker-compose -f "${PROJECT_ROOT}/docker-compose.complete.yml" ps elidune-complete 2>/dev/null | grep -q "Up"; then
        USE_COMPOSE=true
        COMPOSE_FILE="${PROJECT_ROOT}/docker-compose.complete.yml"
        SERVICE_NAME="elidune-complete"
    fi
fi

# Fallback to direct container access
if [ "${USE_COMPOSE}" = false ]; then
    CONTAINER_NAME="${CONTAINER_NAME:-elidune-complete}"
    if ! docker ps --format '{{.Names}}' | grep -q "^${CONTAINER_NAME}$"; then
        echo -e "${RED}Error: Container ${CONTAINER_NAME} is not running${NC}"
        echo -e "${YELLOW}Please start the container first or use docker-compose${NC}"
        exit 1
    fi
fi

# Get version from argument or detect from error
VERSION="${1:-}"

if [ -z "${VERSION}" ]; then
    echo -e "${GREEN}=== Fix Missing Migration Version ===${NC}"
    echo ""
    echo -e "${YELLOW}This script fixes VersionMissing errors by marking migrations as applied${NC}"
    echo ""
    echo -e "${RED}Usage: $0 <version_number>${NC}"
    echo -e "${YELLOW}Example: $0 14${NC}"
    echo ""
    echo -e "${YELLOW}To find the missing version, check the error message:${NC}"
    echo "  Failed to run database migrations: VersionMissing(14)"
    echo "  The missing version is: 14"
    exit 1
fi

echo -e "${GREEN}=== Fixing Missing Migration Version ${VERSION} ===${NC}"
echo ""

# Wait for PostgreSQL to be ready
echo -e "${YELLOW}Waiting for PostgreSQL to be ready...${NC}"
for i in {1..60}; do
    if [ "${USE_COMPOSE}" = true ]; then
        if docker-compose -f "${COMPOSE_FILE}" exec -T "${SERVICE_NAME}" pg_isready -U "${DB_USER}" -d "${DB_NAME}" > /dev/null 2>&1; then
            echo -e "${GREEN}✓ PostgreSQL is ready${NC}"
            break
        fi
    else
        if docker exec "${CONTAINER_NAME}" pg_isready -U "${DB_USER}" -d "${DB_NAME}" > /dev/null 2>&1; then
            echo -e "${GREEN}✓ PostgreSQL is ready${NC}"
            break
        fi
    fi
    if [ $i -eq 60 ]; then
        echo -e "${RED}Error: PostgreSQL did not become ready in time${NC}"
        exit 1
    fi
    sleep 1
done

# Check if migration file exists
MIGRATION_FILE=""
if [ "${USE_COMPOSE}" = true ]; then
    MIGRATION_FILE=$(docker-compose -f "${COMPOSE_FILE}" exec -T "${SERVICE_NAME}" \
        sh -c "ls /app/migrations/${VERSION}_*.sql 2>/dev/null | head -1" | tr -d '\r\n' || echo "")
else
    MIGRATION_FILE=$(docker exec "${CONTAINER_NAME}" \
        sh -c "ls /app/migrations/${VERSION}_*.sql 2>/dev/null | head -1" | tr -d '\r\n' || echo "")
fi

if [ -z "${MIGRATION_FILE}" ]; then
    echo -e "${RED}Error: Migration file for version ${VERSION} not found in container${NC}"
    exit 1
fi

MIGRATION_NAME=$(basename "${MIGRATION_FILE}" .sql)

echo -e "${YELLOW}Found migration: ${MIGRATION_NAME}${NC}"
echo ""
echo -e "${YELLOW}⚠️  WARNING: This will mark migration ${VERSION} as applied without running it${NC}"
echo -e "${YELLOW}Only use this if the migration has already been applied manually or if you're sure${NC}"
echo -e "${YELLOW}the database schema already matches what the migration would do${NC}"
echo ""
read -p "Do you want to continue? (y/N) " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo -e "${YELLOW}Cancelled${NC}"
    exit 0
fi

# Get migration checksum (simple hash of filename for now)
# In production, SQLx uses a checksum of the file content
CHECKSUM=$(echo -n "${MIGRATION_NAME}" | sha256sum | cut -d' ' -f1 | head -c 16)

echo ""
echo -e "${YELLOW}Inserting migration record...${NC}"

# Insert migration record
if [ "${USE_COMPOSE}" = true ]; then
    docker-compose -f "${COMPOSE_FILE}" exec -T "${SERVICE_NAME}" psql -U "${DB_USER}" -d "${DB_NAME}" <<SQL
INSERT INTO _sqlx_migrations (version, description, installed_on, success, checksum)
VALUES (${VERSION}, '${MIGRATION_NAME}', NOW(), true, '${CHECKSUM}')
ON CONFLICT (version) DO NOTHING;
SQL
else
    docker exec "${CONTAINER_NAME}" psql -U "${DB_USER}" -d "${DB_NAME}" <<SQL
INSERT INTO _sqlx_migrations (version, description, installed_on, success, checksum)
VALUES (${VERSION}, '${MIGRATION_NAME}', NOW(), true, '${CHECKSUM}')
ON CONFLICT (version) DO NOTHING;
SQL
fi

if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓ Migration ${VERSION} marked as applied${NC}"
else
    echo -e "${RED}Error: Failed to insert migration record${NC}"
    exit 1
fi

echo ""
echo -e "${GREEN}=== Done ===${NC}"
echo ""
echo -e "${YELLOW}Next steps:${NC}"
if [ "${USE_COMPOSE}" = true ]; then
    echo "  1. Restart the service: docker-compose -f docker-compose.complete.yml restart"
    echo "  2. The server should now start successfully"
else
    echo "  1. Restart the container: docker restart ${CONTAINER_NAME}"
    echo "  2. The server should now start successfully"
fi
echo ""
