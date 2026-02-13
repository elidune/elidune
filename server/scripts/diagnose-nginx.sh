#!/bin/bash

# Script to diagnose Nginx configuration issues
# Usage: ./scripts/diagnose-nginx.sh

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}=== Nginx Configuration Diagnosis ===${NC}"
echo ""

# Check if nginx is running
echo -e "${YELLOW}1. Checking Nginx status...${NC}"
if systemctl is-active --quiet nginx; then
    echo -e "${GREEN}✓ Nginx is running${NC}"
else
    echo -e "${RED}✗ Nginx is not running${NC}"
    exit 1
fi

# Check if configuration file exists
echo ""
echo -e "${YELLOW}2. Checking configuration files...${NC}"
if [ -f "/etc/nginx/sites-available/elidune.b-612.fr" ]; then
    echo -e "${GREEN}✓ Configuration file exists: /etc/nginx/sites-available/elidune.b-612.fr${NC}"
else
    echo -e "${RED}✗ Configuration file not found: /etc/nginx/sites-available/elidune.b-612.fr${NC}"
fi

if [ -L "/etc/nginx/sites-enabled/elidune.b-612.fr" ]; then
    echo -e "${GREEN}✓ Symlink exists: /etc/nginx/sites-enabled/elidune.b-612.fr${NC}"
    echo -e "${YELLOW}  Points to: $(readlink -f /etc/nginx/sites-enabled/elidune.b-612.fr)${NC}"
else
    echo -e "${RED}✗ Symlink not found: /etc/nginx/sites-enabled/elidune.b-612.fr${NC}"
fi

# Check nginx configuration
echo ""
echo -e "${YELLOW}3. Testing Nginx configuration...${NC}"
if sudo nginx -t 2>&1 | grep -q "successful"; then
    echo -e "${GREEN}✓ Nginx configuration is valid${NC}"
else
    echo -e "${RED}✗ Nginx configuration has errors:${NC}"
    sudo nginx -t
fi

# Check if elidune.b-612.fr is in the config
echo ""
echo -e "${YELLOW}4. Checking server_name in configuration...${NC}"
if grep -q "server_name elidune.b-612.fr" /etc/nginx/sites-available/elidune.b-612.fr 2>/dev/null; then
    echo -e "${GREEN}✓ server_name elidune.b-612.fr found in config${NC}"
else
    echo -e "${RED}✗ server_name elidune.b-612.fr not found${NC}"
fi

# Check if default site is interfering
echo ""
echo -e "${YELLOW}5. Checking for default nginx site...${NC}"
if [ -L "/etc/nginx/sites-enabled/default" ]; then
    echo -e "${YELLOW}⚠ Default site is enabled - this might interfere${NC}"
    echo -e "${YELLOW}  Consider disabling it: sudo rm /etc/nginx/sites-enabled/default${NC}"
else
    echo -e "${GREEN}✓ Default site is not enabled${NC}"
fi

# Check if ports are accessible
echo ""
echo -e "${YELLOW}6. Checking if Docker container ports are accessible...${NC}"
if curl -s -o /dev/null -w "%{http_code}" http://localhost:8181 | grep -q "200\|301\|302"; then
    echo -e "${GREEN}✓ Port 8181 (GUI) is accessible${NC}"
else
    echo -e "${RED}✗ Port 8181 (GUI) is not accessible${NC}"
    echo -e "${YELLOW}  Check if Docker container is running: docker ps | grep elidune${NC}"
fi

if curl -s -o /dev/null -w "%{http_code}" http://localhost:8282/api/v1/health | grep -q "200\|404"; then
    echo -e "${GREEN}✓ Port 8282 (API) is accessible${NC}"
else
    echo -e "${RED}✗ Port 8282 (API) is not accessible${NC}"
fi

# Show active server blocks
echo ""
echo -e "${YELLOW}7. Active Nginx server blocks:${NC}"
ls -la /etc/nginx/sites-enabled/ | grep -v "^total" | awk '{print $9, $10, $11}'

# Show nginx error log (last 10 lines)
echo ""
echo -e "${YELLOW}8. Last 10 lines of Nginx error log:${NC}"
sudo tail -10 /var/log/nginx/error.log 2>/dev/null || echo "No error log found"

# Show nginx access log for elidune
echo ""
echo -e "${YELLOW}9. Recent access logs for elidune.b-612.fr:${NC}"
if [ -f "/var/log/nginx/elidune-access.log" ]; then
    sudo tail -5 /var/log/nginx/elidune-access.log
else
    echo "No elidune access log found"
fi

echo ""
echo -e "${GREEN}=== Diagnosis complete ===${NC}"
echo ""
echo -e "${YELLOW}Next steps:${NC}"
echo "  1. If config file is missing, run: sudo ./scripts/setup-nginx-host.sh"
echo "  2. If symlink is missing, run: sudo ln -s /etc/nginx/sites-available/elidune.b-612.fr /etc/nginx/sites-enabled/"
echo "  3. If default site interferes, disable it: sudo rm /etc/nginx/sites-enabled/default"
echo "  4. Reload nginx: sudo systemctl reload nginx"
echo ""
