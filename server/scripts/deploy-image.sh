#!/bin/bash
#
# Deploy Elidune Docker image to a remote host.
# Usage: ./scripts/deploy-image.sh user@host:/repertoire_cible
# Example: ./scripts/deploy-image.sh deploy@myserver:/opt/elidune
#
set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

IMAGE_NAME="${IMAGE_NAME:-elidune-complete}"
IMAGE_TAG="${IMAGE_TAG:-latest}"
CONTAINER_NAME="${CONTAINER_NAME:-elidune-complete}"
TAR_NAME="elidune-complete-${IMAGE_TAG}.tar"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

if [ -z "$1" ]; then
    echo -e "${RED}Usage: $0 user@host:/repertoire_cible${NC}"
    echo "Example: $0 deploy@myserver:/opt/elidune"
    exit 1
fi

TARGET="$1"
# Parse user@host and path
if [[ "$TARGET" =~ ^([^:]+):(.+)$ ]]; then
    SSH_TARGET="${BASH_REMATCH[1]}"
    REMOTE_DIR="${BASH_REMATCH[2]}"
else
    echo -e "${RED}Invalid target. Use user@host:/path${NC}"
    exit 1
fi

echo -e "${GREEN}=== Elidune image deployment ===${NC}"
echo "Target: ${SSH_TARGET}:${REMOTE_DIR}"
echo ""

# 1. Build image
echo -e "${YELLOW}[1/4] Building image...${NC}"
"${SCRIPT_DIR}/build-complete-image.sh"
echo ""

# 2. Save image to tar
echo -e "${YELLOW}[2/4] Saving image to ${TAR_NAME}...${NC}"
docker save "${IMAGE_NAME}:${IMAGE_TAG}" -o "${PROJECT_ROOT}/${TAR_NAME}"
echo -e "${GREEN}✓ Image saved${NC}"
echo ""

# 3. Copy tar and docker-compose.complete.yml (if missing) to remote
echo -e "${YELLOW}[3/5] Copying image to ${SSH_TARGET}...${NC}"
scp "${PROJECT_ROOT}/${TAR_NAME}" "${SSH_TARGET}:${REMOTE_DIR}/"
if ! ssh "${SSH_TARGET}" "[ -f ${REMOTE_DIR}/docker-compose.complete.yml ]"; then
    echo -e "${YELLOW}Copying docker-compose.complete.yml (not present on remote)...${NC}"
    scp "${PROJECT_ROOT}/docker-compose.complete.yml" "${SSH_TARGET}:${REMOTE_DIR}/"
fi
echo ""

echo -e "${YELLOW}[4/4] Installing and restarting on remote...${NC}"
ssh "${SSH_TARGET}" "cd ${REMOTE_DIR} && \
    sudo docker stop ${CONTAINER_NAME} 2>/dev/null || true && \
    sudo docker rm ${CONTAINER_NAME} 2>/dev/null || true && \
    sudo docker rmi ${IMAGE_NAME}:${IMAGE_TAG} 2>/dev/null || true && \
    sudo docker load -i ${TAR_NAME} && \
    rm -f ${TAR_NAME} && \
    if [ -f docker-compose.complete.yml ]; then \
        ELIDUNE_IMAGE=${IMAGE_NAME}:${IMAGE_TAG} sudo docker compose -f docker-compose.complete.yml up -d; \
    else \
        sudo docker volume create elidune-postgres-data 2>/dev/null || true; \
        sudo docker volume create elidune-redis-data 2>/dev/null || true; \
        sudo docker run -d --name ${CONTAINER_NAME} \
            -p 5433:5432 -p 6379:6379 -p 8282:8080 -p 8181:80 \
            -v elidune-postgres-data:/var/lib/postgresql/data \
            -v elidune-redis-data:/data \
            --restart unless-stopped \
            ${IMAGE_NAME}:${IMAGE_TAG}; \
    fi"

# Remove local tar
rm -f "${PROJECT_ROOT}/${TAR_NAME}"

echo ""
echo -e "${GREEN}=== Deployment complete ===${NC}"
echo "Remote instance restarted. Check with: ssh ${SSH_TARGET} 'docker ps'"
