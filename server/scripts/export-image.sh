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
EXPORT_FILE="${EXPORT_FILE:-elidune-complete-$(date +%Y%m%d-%H%M%S).tar.gz}"
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

echo -e "${GREEN}=== Exporting Docker image ===${NC}"

# Check if image exists
if ! docker image inspect "${IMAGE_NAME}:${IMAGE_TAG}" > /dev/null 2>&1; then
    echo -e "${RED}Error: Image ${IMAGE_NAME}:${IMAGE_TAG} not found${NC}"
    echo -e "${YELLOW}Please build the image first with: ./scripts/build-db-image.sh${NC}"
    exit 1
fi

echo -e "${YELLOW}Exporting ${IMAGE_NAME}:${IMAGE_TAG} to ${EXPORT_FILE}...${NC}"

# Save image to tar file and compress
docker save "${IMAGE_NAME}:${IMAGE_TAG}" | gzip > "${PROJECT_ROOT}/${EXPORT_FILE}"

if [ $? -eq 0 ]; then
    FILE_SIZE=$(du -h "${PROJECT_ROOT}/${EXPORT_FILE}" | cut -f1)
    echo -e "${GREEN}✓ Image exported successfully${NC}"
    echo -e "${GREEN}File: ${PROJECT_ROOT}/${EXPORT_FILE} (${FILE_SIZE})${NC}"
    echo ""
    echo -e "${YELLOW}To transfer to another machine:${NC}"
    echo "  scp ${EXPORT_FILE} user@remote-host:/path/to/destination/"
    echo ""
    echo -e "${YELLOW}On the remote machine, load the image with:${NC}"
    echo "  gunzip -c ${EXPORT_FILE} | docker load"
    echo ""
    echo -e "${YELLOW}Or use the import script:${NC}"
    echo "  ./scripts/import-image.sh ${EXPORT_FILE}"
else
    echo -e "${RED}Error: Failed to export image${NC}"
    exit 1
fi

echo -e "${GREEN}=== Done ===${NC}"
