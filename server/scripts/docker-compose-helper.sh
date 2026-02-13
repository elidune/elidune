#!/bin/bash

# Helper script for docker-compose operations
# Usage: ./scripts/docker-compose-helper.sh [command]

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
COMPOSE_FILE="${PROJECT_ROOT}/docker-compose.complete.yml"
ENV_FILE="${PROJECT_ROOT}/.env"

# Check if .env exists
if [ ! -f "${ENV_FILE}" ]; then
    echo -e "${YELLOW}Warning: .env file not found${NC}"
    echo -e "${YELLOW}Creating .env from .env.example...${NC}"
    if [ -f "${PROJECT_ROOT}/.env.example" ]; then
        cp "${PROJECT_ROOT}/.env.example" "${ENV_FILE}"
        echo -e "${GREEN}✓ .env file created${NC}"
        echo -e "${YELLOW}⚠️  Please edit .env and set JWT_SECRET and other values before starting${NC}"
    else
        echo -e "${RED}Error: .env.example not found${NC}"
        exit 1
    fi
fi

# Function to check if service is running
is_running() {
    docker-compose -f "${COMPOSE_FILE}" ps elidune-complete 2>/dev/null | grep -q "Up"
}

# Function to show status
show_status() {
    echo -e "${GREEN}=== Elidune Complete Status ===${NC}"
    echo ""
    docker-compose -f "${COMPOSE_FILE}" ps
    echo ""
    
    if is_running; then
        echo -e "${GREEN}Service is running${NC}"
        echo ""
        echo -e "${YELLOW}Access:${NC}"
        
        # Read ports from .env or use defaults
        GUI_PORT=$(grep "^GUI_PORT=" "${ENV_FILE}" 2>/dev/null | cut -d= -f2 || echo "8181")
        API_PORT=$(grep "^API_PORT=" "${ENV_FILE}" 2>/dev/null | cut -d= -f2 || echo "8282")
        POSTGRES_PORT=$(grep "^POSTGRES_PORT=" "${ENV_FILE}" 2>/dev/null | cut -d= -f2 || echo "5433")
        
        echo "  - Web UI: http://localhost:${GUI_PORT}"
        echo "  - API: http://localhost:${API_PORT}"
        echo "  - PostgreSQL: localhost:${POSTGRES_PORT}"
        echo "  - Redis: localhost:6379"
    else
        echo -e "${YELLOW}Service is not running${NC}"
    fi
}

# Main command handling
case "${1:-}" in
    start|up)
        echo -e "${GREEN}Starting Elidune Complete...${NC}"
        docker-compose -f "${COMPOSE_FILE}" up -d
        echo ""
        sleep 2
        show_status
        ;;
    stop)
        echo -e "${YELLOW}Stopping Elidune Complete...${NC}"
        docker-compose -f "${COMPOSE_FILE}" stop
        echo -e "${GREEN}✓ Service stopped${NC}"
        ;;
    restart)
        echo -e "${YELLOW}Restarting Elidune Complete...${NC}"
        docker-compose -f "${COMPOSE_FILE}" restart
        echo ""
        sleep 2
        show_status
        ;;
    down)
        echo -e "${YELLOW}Stopping and removing Elidune Complete...${NC}"
        echo -e "${YELLOW}⚠️  Volumes will persist (data is safe)${NC}"
        docker-compose -f "${COMPOSE_FILE}" down
        echo -e "${GREEN}✓ Service stopped and removed${NC}"
        ;;
    down-v)
        echo -e "${RED}⚠️  WARNING: This will remove volumes and delete all data!${NC}"
        read -p "Are you sure? (y/N) " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            docker-compose -f "${COMPOSE_FILE}" down -v
            echo -e "${GREEN}✓ Service and volumes removed${NC}"
        else
            echo -e "${YELLOW}Cancelled${NC}"
        fi
        ;;
    logs)
        docker-compose -f "${COMPOSE_FILE}" logs -f
        ;;
    status|ps)
        show_status
        ;;
    shell|exec)
        if is_running; then
            docker-compose -f "${COMPOSE_FILE}" exec elidune-complete sh
        else
            echo -e "${RED}Error: Service is not running${NC}"
            exit 1
        fi
        ;;
    dump|export)
        if is_running; then
            "${PROJECT_ROOT}/scripts/dump-db.sh"
        else
            echo -e "${RED}Error: Service is not running${NC}"
            exit 1
        fi
        ;;
    import)
        if [ -z "${2:-}" ]; then
            echo -e "${RED}Usage: $0 import <dump-file.sql.gz>${NC}"
            exit 1
        fi
        if is_running; then
            "${PROJECT_ROOT}/scripts/import-db.sh" "${2}"
        else
            echo -e "${RED}Error: Service is not running${NC}"
            exit 1
        fi
        ;;
    backup-volumes)
        "${PROJECT_ROOT}/scripts/backup-volumes.sh"
        ;;
    *)
        echo -e "${GREEN}Elidune Complete Docker Compose Helper${NC}"
        echo ""
        echo "Usage: $0 [command]"
        echo ""
        echo "Commands:"
        echo "  start, up          Start the service"
        echo "  stop                Stop the service"
        echo "  restart             Restart the service"
        echo "  down                Stop and remove container (volumes persist)"
        echo "  down-v              Stop and remove container AND volumes (⚠️ deletes data)"
        echo "  logs                Show and follow logs"
        echo "  status, ps          Show service status"
        echo "  shell, exec         Open shell in container"
        echo "  dump, export        Export database"
        echo "  import <file>      Import database dump"
        echo "  backup-volumes      Backup Docker volumes"
        echo ""
        echo "Examples:"
        echo "  $0 start"
        echo "  $0 logs"
        echo "  $0 dump"
        echo "  $0 import elidune-db-dump-20260213-143653.sql.gz"
        exit 1
        ;;
esac
