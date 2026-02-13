#!/bin/bash

# Script to setup Nginx configuration on host server for Elidune
# Usage: ./scripts/setup-nginx-host.sh

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
NGINX_CONFIG_SOURCE="${PROJECT_ROOT}/docker/nginx-host-elidune.conf"
NGINX_SITES_AVAILABLE="/etc/nginx/sites-available"
NGINX_SITES_ENABLED="/etc/nginx/sites-enabled"
SITE_NAME="elidune.b-612.fr"

echo -e "${GREEN}=== Setting up Nginx configuration for Elidune ===${NC}"
echo ""

# Check if running as root
if [ "$EUID" -ne 0 ]; then
    echo -e "${RED}Error: This script must be run as root${NC}"
    echo -e "${YELLOW}Use: sudo $0${NC}"
    exit 1
fi

# Check if config file exists
if [ ! -f "${NGINX_CONFIG_SOURCE}" ]; then
    echo -e "${RED}Error: Nginx config file not found: ${NGINX_CONFIG_SOURCE}${NC}"
    exit 1
fi

# Check if nginx is installed
if ! command -v nginx &> /dev/null; then
    echo -e "${RED}Error: Nginx is not installed${NC}"
    echo -e "${YELLOW}Install it with: sudo apt-get install nginx${NC}"
    exit 1
fi

# Create sites-available directory if it doesn't exist
mkdir -p "${NGINX_SITES_AVAILABLE}"
mkdir -p "${NGINX_SITES_ENABLED}"

# Copy configuration file
echo -e "${YELLOW}Copying Nginx configuration...${NC}"
cp "${NGINX_CONFIG_SOURCE}" "${NGINX_SITES_AVAILABLE}/${SITE_NAME}"
echo -e "${GREEN}✓ Configuration copied to ${NGINX_SITES_AVAILABLE}/${SITE_NAME}${NC}"

# Check if symlink already exists
if [ -L "${NGINX_SITES_ENABLED}/${SITE_NAME}" ]; then
    echo -e "${YELLOW}Symlink already exists, removing old one...${NC}"
    rm "${NGINX_SITES_ENABLED}/${SITE_NAME}"
fi

# Create symlink
echo -e "${YELLOW}Creating symlink...${NC}"
ln -s "${NGINX_SITES_AVAILABLE}/${SITE_NAME}" "${NGINX_SITES_ENABLED}/${SITE_NAME}"
echo -e "${GREEN}✓ Symlink created${NC}"

# Test nginx configuration
echo ""
echo -e "${YELLOW}Testing Nginx configuration...${NC}"
if nginx -t; then
    echo -e "${GREEN}✓ Nginx configuration is valid${NC}"
else
    echo -e "${RED}Error: Nginx configuration test failed${NC}"
    exit 1
fi

# Ask to reload nginx
echo ""
read -p "Do you want to reload Nginx now? (y/N) " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo -e "${YELLOW}Reloading Nginx...${NC}"
    systemctl reload nginx
    echo -e "${GREEN}✓ Nginx reloaded${NC}"
else
    echo -e "${YELLOW}You can reload Nginx manually with:${NC}"
    echo "  sudo systemctl reload nginx"
fi

echo ""
echo -e "${GREEN}=== Setup completed ===${NC}"
echo ""
echo -e "${YELLOW}Configuration file: ${NGINX_SITES_AVAILABLE}/${SITE_NAME}${NC}"
echo -e "${YELLOW}Symlink: ${NGINX_SITES_ENABLED}/${SITE_NAME}${NC}"
echo ""
echo -e "${YELLOW}⚠️  Important:${NC}"
echo "  1. Verify SSL certificate paths in the config file"
echo "  2. Ensure ports 8181 and 8282 are accessible from localhost"
echo "  3. Test the configuration: curl -I https://elidune.b-612.fr"
echo ""
