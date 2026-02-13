#!/bin/bash

# Script to prepare deployment archive for Elidune Complete
# Usage: ./scripts/prepare-deployment.sh [output-directory]

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUTPUT_DIR="${1:-${PROJECT_ROOT}}"
TIMESTAMP=$(date +%Y%m%d-%H%M%S)
ARCHIVE_NAME="elidune-deploy-${TIMESTAMP}.tar.gz"
TEMP_DIR=$(mktemp -d)

echo -e "${GREEN}=== Preparing Elidune Deployment Archive ===${NC}"
echo ""

# Create temporary directory structure
mkdir -p "${TEMP_DIR}/docker"
mkdir -p "${TEMP_DIR}/scripts"

# Copy essential files
echo -e "${YELLOW}Copying essential files...${NC}"

# Docker Compose
if [ -f "${PROJECT_ROOT}/docker-compose.complete.yml" ]; then
    cp "${PROJECT_ROOT}/docker-compose.complete.yml" "${TEMP_DIR}/"
    echo "  ✓ docker-compose.complete.yml"
else
    echo -e "${RED}  ✗ docker-compose.complete.yml not found${NC}"
    exit 1
fi

# Environment template
if [ -f "${PROJECT_ROOT}/.env.example" ]; then
    cp "${PROJECT_ROOT}/.env.example" "${TEMP_DIR}/"
    echo "  ✓ .env.example"
else
    echo -e "${RED}  ✗ .env.example not found${NC}"
    exit 1
fi

# Docker configuration files
if [ -f "${PROJECT_ROOT}/docker/nginx-complete.conf" ]; then
    cp "${PROJECT_ROOT}/docker/nginx-complete.conf" "${TEMP_DIR}/docker/"
    echo "  ✓ docker/nginx-complete.conf"
else
    echo -e "${RED}  ✗ docker/nginx-complete.conf not found${NC}"
    exit 1
fi

if [ -f "${PROJECT_ROOT}/docker/supervisord.conf" ]; then
    cp "${PROJECT_ROOT}/docker/supervisord.conf" "${TEMP_DIR}/docker/"
    echo "  ✓ docker/supervisord.conf"
else
    echo -e "${RED}  ✗ docker/supervisord.conf not found${NC}"
    exit 1
fi

if [ -f "${PROJECT_ROOT}/docker/wait-and-start-server.sh" ]; then
    cp "${PROJECT_ROOT}/docker/wait-and-start-server.sh" "${TEMP_DIR}/docker/"
    chmod +x "${TEMP_DIR}/docker/wait-and-start-server.sh"
    echo "  ✓ docker/wait-and-start-server.sh"
else
    echo -e "${RED}  ✗ docker/wait-and-start-server.sh not found${NC}"
    exit 1
fi

# Utility scripts
echo ""
echo -e "${YELLOW}Copying utility scripts...${NC}"

SCRIPTS=(
    "dump-db.sh"
    "import-db.sh"
    "backup-volumes.sh"
    "restore-volumes.sh"
    "docker-compose-helper.sh"
)

for script in "${SCRIPTS[@]}"; do
    if [ -f "${PROJECT_ROOT}/scripts/${script}" ]; then
        cp "${PROJECT_ROOT}/scripts/${script}" "${TEMP_DIR}/scripts/"
        chmod +x "${TEMP_DIR}/scripts/${script}"
        echo "  ✓ scripts/${script}"
    else
        echo -e "${YELLOW}  ⚠ scripts/${script} not found (optional)${NC}"
    fi
done

# Optional: Dockerfile for building on server
echo ""
echo -e "${YELLOW}Copying optional build files...${NC}"

if [ -f "${PROJECT_ROOT}/Dockerfile.complete" ]; then
    cp "${PROJECT_ROOT}/Dockerfile.complete" "${TEMP_DIR}/"
    echo "  ✓ Dockerfile.complete"
    
    # Copy build script if exists
    if [ -f "${PROJECT_ROOT}/scripts/build-complete-image.sh" ]; then
        cp "${PROJECT_ROOT}/scripts/build-complete-image.sh" "${TEMP_DIR}/scripts/"
        chmod +x "${TEMP_DIR}/scripts/build-complete-image.sh"
        echo "  ✓ scripts/build-complete-image.sh"
    fi
else
    echo -e "${YELLOW}  ⚠ Dockerfile.complete not found (build on server won't be possible)${NC}"
fi

# Copy README
if [ -f "${PROJECT_ROOT}/README-docker.md" ]; then
    cp "${PROJECT_ROOT}/README-docker.md" "${TEMP_DIR}/"
    echo "  ✓ README-docker.md"
fi

# Create archive
echo ""
echo -e "${YELLOW}Creating archive...${NC}"
cd "${TEMP_DIR}"
tar czf "${OUTPUT_DIR}/${ARCHIVE_NAME}" .

# Cleanup
cd - > /dev/null
rm -rf "${TEMP_DIR}"

# Show archive info
ARCHIVE_SIZE=$(du -h "${OUTPUT_DIR}/${ARCHIVE_NAME}" | cut -f1)

echo ""
echo -e "${GREEN}=== Archive created successfully ===${NC}"
echo ""
echo -e "${GREEN}Archive: ${OUTPUT_DIR}/${ARCHIVE_NAME}${NC}"
echo -e "${GREEN}Size: ${ARCHIVE_SIZE}${NC}"
echo ""
echo -e "${YELLOW}To deploy on remote server:${NC}"
echo ""
echo "  1. Transfer the archive:"
echo "     scp ${OUTPUT_DIR}/${ARCHIVE_NAME} user@server:/opt/elidune/"
echo ""
echo "  2. On the server, extract:"
echo "     cd /opt/elidune"
echo "     tar xzf ${ARCHIVE_NAME}"
echo ""
echo "  3. Configure:"
echo "     cp .env.example .env"
echo "     nano .env  # Edit JWT_SECRET and other settings"
echo ""
echo "  4. Load Docker image (if you have one):"
echo "     gunzip -c elidune-complete-image.tar.gz | docker load"
echo ""
echo "  5. Start the service:"
echo "     docker-compose -f docker-compose.complete.yml up -d"
echo ""
echo -e "${YELLOW}See README-docker.md for detailed instructions${NC}"
