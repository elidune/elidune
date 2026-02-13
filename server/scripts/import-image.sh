#!/bin/bash

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

if [ $# -eq 0 ]; then
    echo -e "${RED}Usage: $0 <image-file.tar.gz>${NC}"
    echo -e "${YELLOW}Example: $0 elidune-complete-20260212-143653.tar.gz${NC}"
    exit 1
fi

EXPORT_FILE="$1"
IMAGE_NAME="${IMAGE_NAME:-elidune-complete}"
IMAGE_TAG="${IMAGE_TAG:-latest}"

if [ ! -f "${EXPORT_FILE}" ]; then
    echo -e "${RED}Error: File ${EXPORT_FILE} not found${NC}"
    exit 1
fi

echo -e "${GREEN}=== Importing Docker image ===${NC}"
echo -e "${YELLOW}Loading ${EXPORT_FILE}...${NC}"

# Load image from tar file
gunzip -c "${EXPORT_FILE}" | docker load

if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓ Image imported successfully${NC}"
    echo ""
    echo -e "${YELLOW}To run the container:${NC}"
    echo "  docker run -d --name elidune-complete -p 5432:5432 -p 8080:8080 ${IMAGE_NAME}:${IMAGE_TAG}"
    echo ""
    echo -e "${YELLOW}With custom environment variables:${NC}"
    echo "  docker run -d --name elidune-complete \\"
    echo "    -p 5432:5432 -p 8080:8080 \\"
    echo "    -e JWT_SECRET=your-secret-key \\"
    echo "    -e RUST_LOG=elidune_server=debug \\"
    echo "    ${IMAGE_NAME}:${IMAGE_TAG}"
    echo ""
    echo -e "${YELLOW}To access:${NC}"
    echo "  - API: http://localhost:8080"
    echo "  - PostgreSQL: localhost:5432"
else
    echo -e "${RED}Error: Failed to import image${NC}"
    exit 1
fi

echo -e "${GREEN}=== Done ===${NC}"
