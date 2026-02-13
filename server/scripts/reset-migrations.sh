#!/bin/bash

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

echo -e "${GREEN}=== Resetting Database Migrations ===${NC}"
echo ""

if [ "${USE_COMPOSE}" = true ]; then
    echo -e "${YELLOW}Using docker-compose mode${NC}"
else
    echo -e "${YELLOW}Using direct container access${NC}"
fi

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

echo ""
echo -e "${YELLOW}⚠️  WARNING: This will reset the migration tracking table${NC}"
echo -e "${YELLOW}The migrations will be re-run automatically on next server start${NC}"
echo ""
read -p "Do you want to continue? (y/N) " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo -e "${YELLOW}Cancelled${NC}"
    exit 0
fi

echo ""
echo -e "${YELLOW}Resetting migrations table...${NC}"

# Reset migrations table
if [ "${USE_COMPOSE}" = true ]; then
    docker-compose -f "${COMPOSE_FILE}" exec -T "${SERVICE_NAME}" psql -U "${DB_USER}" -d "${DB_NAME}" <<SQL
-- Truncate or drop the migrations table
TRUNCATE TABLE _sqlx_migrations;
SQL
    
    if [ $? -ne 0 ]; then
        echo -e "${YELLOW}Trying to drop and recreate the table...${NC}"
        docker-compose -f "${COMPOSE_FILE}" exec -T "${SERVICE_NAME}" psql -U "${DB_USER}" -d "${DB_NAME}" <<SQL
DROP TABLE IF EXISTS _sqlx_migrations;
SQL
    fi
else
    docker exec "${CONTAINER_NAME}" psql -U "${DB_USER}" -d "${DB_NAME}" <<SQL
-- Truncate or drop the migrations table
TRUNCATE TABLE _sqlx_migrations;
SQL
    
    if [ $? -ne 0 ]; then
        echo -e "${YELLOW}Trying to drop and recreate the table...${NC}"
        docker exec "${CONTAINER_NAME}" psql -U "${DB_USER}" -d "${DB_NAME}" <<SQL
DROP TABLE IF EXISTS _sqlx_migrations;
SQL
    fi
fi

echo -e "${GREEN}✓ Migrations table reset successfully${NC}"

echo ""
echo -e "${GREEN}=== Done ===${NC}"
echo ""
echo -e "${YELLOW}Next steps:${NC}"
if [ "${USE_COMPOSE}" = true ]; then
    echo "  1. Restart the service: docker-compose -f docker-compose.complete.yml restart"
    echo "  2. The server will automatically re-run all migrations on startup"
else
    echo "  1. Restart the container: docker restart ${CONTAINER_NAME}"
    echo "  2. The server will automatically re-run all migrations on startup"
fi
echo ""
