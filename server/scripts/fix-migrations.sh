#!/bin/bash

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
CONTAINER_NAME="${CONTAINER_NAME:-elidune-complete}"
DB_USER="${DB_USER:-elidune}"
DB_NAME="${DB_NAME:-elidune}"

echo -e "${GREEN}=== Fixing Database Migrations ===${NC}"
echo ""

# Check if container is running
if ! docker ps --format '{{.Names}}' | grep -q "^${CONTAINER_NAME}$"; then
    echo -e "${RED}Error: Container ${CONTAINER_NAME} is not running${NC}"
    echo -e "${YELLOW}Please start the container first:${NC}"
    echo "  docker start ${CONTAINER_NAME}"
    exit 1
fi

# Wait for PostgreSQL to be ready
echo -e "${YELLOW}Waiting for PostgreSQL to be ready...${NC}"
for i in {1..30}; do
    if docker exec "${CONTAINER_NAME}" pg_isready -U "${DB_USER}" -d "${DB_NAME}" > /dev/null 2>&1; then
        echo -e "${GREEN}✓ PostgreSQL is ready${NC}"
        break
    fi
    if [ $i -eq 30 ]; then
        echo -e "${RED}Error: PostgreSQL did not become ready in time${NC}"
        exit 1
    fi
    sleep 1
done

echo ""
echo -e "${YELLOW}Checking migration status...${NC}"

# Check current migration status
CURRENT_VERSION=$(docker exec "${CONTAINER_NAME}" psql -U "${DB_USER}" -d "${DB_NAME}" -t -c "SELECT MAX(version) FROM _sqlx_migrations;" 2>/dev/null | tr -d ' ' || echo "0")

echo -e "${YELLOW}Current migration version in database: ${CURRENT_VERSION}${NC}"

# List available migrations in container
echo ""
echo -e "${YELLOW}Available migrations in container:${NC}"
docker exec "${CONTAINER_NAME}" ls -1 /app/migrations/ 2>/dev/null | sort -V || echo "No migrations directory found"

echo ""
echo -e "${YELLOW}Options:${NC}"
echo "  1. Reset migrations table (remove all migration records)"
echo "  2. Show migration details"
echo "  3. Exit"
echo ""
read -p "Choose an option (1-3): " choice

case $choice in
    1)
        echo ""
        echo -e "${YELLOW}⚠️  WARNING: This will reset the migration tracking table${NC}"
        echo -e "${YELLOW}The migrations will need to be re-run on next server start${NC}"
        read -p "Are you sure? (y/N) " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            echo ""
            echo -e "${YELLOW}Resetting migrations table...${NC}"
            docker exec "${CONTAINER_NAME}" psql -U "${DB_USER}" -d "${DB_NAME}" -c "TRUNCATE TABLE _sqlx_migrations;" 2>/dev/null || \
            docker exec "${CONTAINER_NAME}" psql -U "${DB_USER}" -d "${DB_NAME}" -c "DROP TABLE IF EXISTS _sqlx_migrations;" 2>/dev/null
            
            echo -e "${GREEN}✓ Migrations table reset${NC}"
            echo ""
            echo -e "${YELLOW}The server will re-run all migrations on next start${NC}"
        else
            echo -e "${YELLOW}Cancelled${NC}"
        fi
        ;;
    2)
        echo ""
        echo -e "${YELLOW}Migration details:${NC}"
        docker exec "${CONTAINER_NAME}" psql -U "${DB_USER}" -d "${DB_NAME}" -c "SELECT * FROM _sqlx_migrations ORDER BY version;" 2>/dev/null || echo "No migrations table found"
        ;;
    3)
        echo -e "${YELLOW}Exiting${NC}"
        exit 0
        ;;
    *)
        echo -e "${RED}Invalid option${NC}"
        exit 1
        ;;
esac

echo ""
echo -e "${GREEN}=== Done ===${NC}"
