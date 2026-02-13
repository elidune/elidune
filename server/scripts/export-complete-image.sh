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
EXPORT_IMAGE="${EXPORT_IMAGE:-true}"
EXPORT_DB="${EXPORT_DB:-false}"
CONTAINER_NAME="${CONTAINER_NAME:-elidune-complete}"
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TIMESTAMP=$(date +%Y%m%d-%H%M%S)
EXPORT_DIR="${PROJECT_ROOT}/exports/complete-${TIMESTAMP}"

echo -e "${GREEN}=== Exporting Complete Elidune Docker Image ===${NC}"
echo -e "${GREEN}(PostgreSQL + Redis + Server + UI)${NC}"
echo ""

# Create export directory
mkdir -p "${EXPORT_DIR}"

# Export Docker image
if [ "${EXPORT_IMAGE}" = "true" ]; then
    echo -e "${YELLOW}Step 1: Exporting Docker image...${NC}"
    
    # Check if image exists
    if ! docker image inspect "${IMAGE_NAME}:${IMAGE_TAG}" > /dev/null 2>&1; then
        echo -e "${RED}Error: Image ${IMAGE_NAME}:${IMAGE_TAG} not found${NC}"
        echo -e "${YELLOW}Please build the image first with: ./scripts/build-complete-image.sh${NC}"
        exit 1
    fi
    
    IMAGE_FILE="${EXPORT_DIR}/elidune-complete-image-${TIMESTAMP}.tar.gz"
    echo -e "${YELLOW}Exporting ${IMAGE_NAME}:${IMAGE_TAG} to ${IMAGE_FILE}...${NC}"
    
    # Save image to tar file and compress
    docker save "${IMAGE_NAME}:${IMAGE_TAG}" | gzip > "${IMAGE_FILE}"
    
    if [ $? -eq 0 ]; then
        FILE_SIZE=$(du -h "${IMAGE_FILE}" | cut -f1)
        echo -e "${GREEN}✓ Image exported successfully (${FILE_SIZE})${NC}"
    else
        echo -e "${RED}Error: Failed to export image${NC}"
        exit 1
    fi
fi

# Export database (optional)
if [ "${EXPORT_DB}" = "true" ]; then
    echo ""
    echo -e "${YELLOW}Step 2: Exporting database...${NC}"
    
    # Check if container is running
    if ! docker ps --format '{{.Names}}' | grep -q "^${CONTAINER_NAME}$"; then
        echo -e "${YELLOW}Warning: Container ${CONTAINER_NAME} is not running${NC}"
        echo -e "${YELLOW}Skipping database export${NC}"
    else
        DB_FILE="${EXPORT_DIR}/elidune-database-${TIMESTAMP}.sql.gz"
        echo -e "${YELLOW}Exporting database from container ${CONTAINER_NAME}...${NC}"
        
        # Export database
        docker exec "${CONTAINER_NAME}" pg_dump -U elidune -d elidune --clean --if-exists --no-owner --no-acl | gzip > "${DB_FILE}"
        
        if [ $? -eq 0 ]; then
            FILE_SIZE=$(du -h "${DB_FILE}" | cut -f1)
            echo -e "${GREEN}✓ Database exported successfully (${FILE_SIZE})${NC}"
        else
            echo -e "${RED}Error: Failed to export database${NC}"
            exit 1
        fi
    fi
fi

# Create README with instructions
cat > "${EXPORT_DIR}/README.md" <<EOF
# Elidune Complete Image Export

Export date: ${TIMESTAMP}

## Contents

$(if [ "${EXPORT_IMAGE}" = "true" ]; then echo "- Docker image: \`elidune-complete-image-${TIMESTAMP}.tar.gz\`"; fi)
$(if [ "${EXPORT_DB}" = "true" ]; then echo "- Database dump: \`elidune-database-${TIMESTAMP}.sql.gz\`"; fi)

## Import Instructions

### 1. Import Docker Image

\`\`\`bash
gunzip -c elidune-complete-image-${TIMESTAMP}.tar.gz | docker load
\`\`\`

Or use the import script:
\`\`\`bash
./scripts/import-complete-image.sh elidune-complete-image-${TIMESTAMP}.tar.gz
\`\`\`

### 2. Run Container

\`\`\`bash
docker run -d --name elidune-complete \\
  -p 5432:5432 \\
  -p 6379:6379 \\
  -p 8080:8080 \\
  -p 80:80 \\
  elidune-complete:latest
\`\`\`

### 3. Import Database (if exported)

Wait for the container to start, then:

\`\`\`bash
gunzip -c elidune-database-${TIMESTAMP}.sql.gz | \\
  docker exec -i elidune-complete psql -U elidune -d elidune
\`\`\`

## Access

- Web UI: http://localhost
- API: http://localhost:8080
- PostgreSQL: localhost:5432
- Redis: localhost:6379

## Default Credentials

- PostgreSQL user: elidune
- PostgreSQL password: elidune
- PostgreSQL database: elidune

⚠️ **Important**: Change the PostgreSQL password in production!
EOF

echo ""
echo -e "${GREEN}=== Export completed successfully ===${NC}"
echo -e "${GREEN}Export directory: ${EXPORT_DIR}${NC}"
echo ""
echo -e "${YELLOW}To transfer to another machine:${NC}"
echo "  scp -r ${EXPORT_DIR} user@remote-host:/path/to/destination/"
echo ""
echo -e "${YELLOW}To import on another machine:${NC}"
echo "  ./scripts/import-complete-image.sh ${EXPORT_DIR}/elidune-complete-image-${TIMESTAMP}.tar.gz"
if [ "${EXPORT_DB}" = "true" ]; then
    echo "  # Then import database:"
    echo "  gunzip -c ${EXPORT_DIR}/elidune-database-${TIMESTAMP}.sql.gz | docker exec -i elidune-complete psql -U elidune -d elidune"
fi
