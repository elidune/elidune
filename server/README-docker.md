# Guide de déploiement Elidune Complete avec Docker Compose

Ce guide explique comment déployer Elidune Complete sur un serveur distant en utilisant Docker Compose avec volumes persistants.

## Table des matières

1. [Prérequis](#prérequis)
2. [Préparation des fichiers](#préparation-des-fichiers)
3. [Transfert vers le serveur distant](#transfert-vers-le-serveur-distant)
4. [Configuration sur le serveur](#configuration-sur-le-serveur)
5. [Build ou import de l'image Docker](#build-ou-import-de-limage-docker)
6. [Démarrage du service](#démarrage-du-service)
7. [Vérification](#vérification)
8. [Gestion des données](#gestion-des-données)
9. [Commandes utiles](#commandes-utiles)
10. [Dépannage](#dépannage)

---

## Prérequis

### Sur le serveur distant

- **Docker** (version 20.10 ou supérieure)
- **Docker Compose** (version 2.0 ou supérieure)
- **Espace disque** : minimum 5 Go (pour l'image + données)
- **RAM** : minimum 2 Go recommandé
- **Ports disponibles** :
  - 5433 (PostgreSQL) - ou autre selon configuration
  - 6379 (Redis) - ou autre selon configuration
  - 8282 (API) - ou autre selon configuration
  - 8181 (GUI) - ou autre selon configuration

### Vérification des prérequis

```bash
# Vérifier Docker
docker --version

# Vérifier Docker Compose
docker-compose --version

# Vérifier l'espace disque
df -h
```

---

## Préparation des fichiers

### Structure de répertoire à créer sur le serveur

```
/opt/elidune/
├── docker-compose.complete.yml
├── .env
├── Dockerfile.complete (optionnel, si build local)
├── docker/
│   ├── nginx-complete.conf
│   ├── supervisord.conf
│   └── wait-and-start-server.sh
└── scripts/
    ├── dump-db.sh
    ├── import-db.sh
    ├── backup-volumes.sh
    ├── restore-volumes.sh
    └── docker-compose-helper.sh
```

### Liste des fichiers à transférer

#### Fichiers essentiels (obligatoires)

1. **`docker-compose.complete.yml`** - Configuration Docker Compose
2. **`.env.example`** - Template de configuration (à copier en `.env`)
3. **`docker/nginx-complete.conf`** - Configuration Nginx interne
4. **`docker/supervisord.conf`** - Configuration Supervisor
5. **`docker/wait-and-start-server.sh`** - Script de démarrage du serveur

#### Fichiers optionnels (pour build local)

6. **`Dockerfile.complete`** - Dockerfile pour construire l'image localement
7. **`scripts/build-complete-image.sh`** - Script de build

#### Scripts utilitaires (recommandés)

8. **`scripts/dump-db.sh`** - Export de la base de données
9. **`scripts/import-db.sh`** - Import de la base de données
10. **`scripts/backup-volumes.sh`** - Sauvegarde des volumes Docker
11. **`scripts/restore-volumes.sh`** - Restauration des volumes Docker
12. **`scripts/docker-compose-helper.sh`** - Helper pour les commandes courantes

---

## Transfert vers le serveur distant

### Option 1 : Transfert via SCP

```bash
# Depuis votre machine locale
cd /home/cjean/Documents/Developments/elidune/elidune-server-rust

# Créer l'archive des fichiers nécessaires
tar czf elidune-deploy.tar.gz \
    docker-compose.complete.yml \
    .env.example \
    Dockerfile.complete \
    docker/ \
    scripts/dump-db.sh \
    scripts/import-db.sh \
    scripts/backup-volumes.sh \
    scripts/restore-volumes.sh \
    scripts/docker-compose-helper.sh

# Transférer vers le serveur
scp elidune-deploy.tar.gz user@serveur-distant:/opt/elidune/

# Sur le serveur distant
ssh user@serveur-distant
cd /opt/elidune
tar xzf elidune-deploy.tar.gz
```

### Option 2 : Transfert via Git

```bash
# Sur le serveur distant
ssh user@serveur-distant
cd /opt
git clone https://github.com/jcollonville/elidune-server-rust.git elidune
cd elidune
```

### Option 3 : Transfert manuel fichier par fichier

```bash
# Créer le répertoire sur le serveur
ssh user@serveur-distant "mkdir -p /opt/elidune/{docker,scripts}"

# Transférer les fichiers un par un
scp docker-compose.complete.yml user@serveur-distant:/opt/elidune/
scp .env.example user@serveur-distant:/opt/elidune/
scp docker/nginx-complete.conf user@serveur-distant:/opt/elidune/docker/
scp docker/supervisord.conf user@serveur-distant:/opt/elidune/docker/
scp docker/wait-and-start-server.sh user@serveur-distant:/opt/elidune/docker/
scp scripts/dump-db.sh user@serveur-distant:/opt/elidune/scripts/
scp scripts/import-db.sh user@serveur-distant:/opt/elidune/scripts/
scp scripts/backup-volumes.sh user@serveur-distant:/opt/elidune/scripts/
scp scripts/restore-volumes.sh user@serveur-distant:/opt/elidune/scripts/
scp scripts/docker-compose-helper.sh user@serveur-distant:/opt/elidune/scripts/
```

### Rendre les scripts exécutables

```bash
# Sur le serveur distant
chmod +x /opt/elidune/docker/wait-and-start-server.sh
chmod +x /opt/elidune/scripts/*.sh
```

---

## Configuration sur le serveur

### 1. Créer le fichier `.env`

```bash
cd /opt/elidune
cp .env.example .env
nano .env  # ou vi .env
```

### 2. Configurer les variables importantes

**⚠️ IMPORTANT : Modifier au minimum ces valeurs :**

```bash
# Générer une clé JWT sécurisée
openssl rand -base64 32

# Éditer .env et remplacer JWT_SECRET
JWT_SECRET=votre-clé-générée-ici

# Modifier le mot de passe PostgreSQL (optionnel mais recommandé)
POSTGRES_PASSWORD=votre-mot-de-passe-securise

# Ajuster les ports si nécessaire
POSTGRES_PORT=5433
API_PORT=8282
GUI_PORT=8181
REDIS_PORT=6379
```

### 3. Vérifier la configuration

```bash
# Vérifier que les ports ne sont pas déjà utilisés
netstat -tuln | grep -E ':(5433|6379|8282|8181)'

# Vérifier que Docker fonctionne
docker ps
```

---

## Build ou import de l'image Docker

### Option A : Importer une image pré-construite (recommandé)

Si vous avez exporté l'image depuis votre machine locale :

```bash
# Sur votre machine locale, exporter l'image
docker save elidune-complete:latest | gzip > elidune-complete-image.tar.gz

# Transférer vers le serveur
scp elidune-complete-image.tar.gz user@serveur-distant:/opt/elidune/

# Sur le serveur distant, charger l'image
cd /opt/elidune
gunzip -c elidune-complete-image.tar.gz | docker load
```

### Option B : Construire l'image sur le serveur

Si vous avez transféré le `Dockerfile.complete` :

```bash
cd /opt/elidune

# Construire l'image (peut prendre 10-30 minutes)
docker build -f Dockerfile.complete -t elidune-complete:latest .

# Ou utiliser le script de build si disponible
./scripts/build-complete-image.sh
```

### Vérifier que l'image est présente

```bash
docker images | grep elidune-complete
```

---

## Démarrage du service

### 1. Démarrer avec Docker Compose

```bash
cd /opt/elidune

# Démarrer en arrière-plan
docker-compose -f docker-compose.complete.yml up -d

# Ou utiliser le helper
./scripts/docker-compose-helper.sh start
```

### 2. Vérifier le démarrage

```bash
# Voir les logs
docker-compose -f docker-compose.complete.yml logs -f

# Ou avec le helper
./scripts/docker-compose-helper.sh logs

# Vérifier le statut
docker-compose -f docker-compose.complete.yml ps
```

### 3. Attendre que les services soient prêts

Le conteneur démarre PostgreSQL, Redis, puis le serveur Elidune. Attendez 30-60 secondes pour que tout soit opérationnel.

---

## Vérification

### 1. Vérifier que le conteneur tourne

```bash
docker ps | grep elidune-complete
```

### 2. Vérifier les logs

```bash
# Logs du serveur Elidune
docker-compose -f docker-compose.complete.yml logs elidune-complete | tail -50

# Logs PostgreSQL
docker-compose -f docker-compose.complete.yml exec elidune-complete tail -f /var/log/supervisor/postgresql.out.log

# Logs du serveur Rust
docker-compose -f docker-compose.complete.yml exec elidune-complete tail -f /var/log/supervisor/elidune-server.out.log
```

### 3. Tester l'accès aux services

```bash
# Tester l'API
curl http://localhost:8282/api/v1/health

# Tester la GUI (devrait retourner du HTML)
curl http://localhost:8181

# Tester PostgreSQL
docker-compose -f docker-compose.complete.yml exec elidune-complete pg_isready -U elidune

# Tester Redis
docker-compose -f docker-compose.complete.yml exec elidune-complete redis-cli ping
```

### 4. Accéder à l'interface web

Ouvrez dans votre navigateur :
- **GUI** : `http://votre-serveur:8181`
- **API** : `http://votre-serveur:8282/api/v1/health`

---

## Gestion des données

### Export de la base de données

```bash
cd /opt/elidune
./scripts/dump-db.sh

# Le dump sera créé dans /opt/elidune/elidune-db-dump-YYYYMMDD-HHMMSS.sql.gz
```

### Import de la base de données

```bash
cd /opt/elidune
./scripts/import-db.sh elidune-db-dump-YYYYMMDD-HHMMSS.sql.gz
```

### Sauvegarde des volumes Docker

```bash
cd /opt/elidune
./scripts/backup-volumes.sh

# Les sauvegardes seront dans ./backups/volumes-YYYYMMDD-HHMMSS/
```

### Restauration des volumes

```bash
cd /opt/elidune
./scripts/restore-volumes.sh ./backups/volumes-YYYYMMDD-HHMMSS
```

---

## Commandes utiles

### Gestion du service

```bash
# Démarrer
docker-compose -f docker-compose.complete.yml up -d
# ou
./scripts/docker-compose-helper.sh start

# Arrêter
docker-compose -f docker-compose.complete.yml stop
# ou
./scripts/docker-compose-helper.sh stop

# Redémarrer
docker-compose -f docker-compose.complete.yml restart
# ou
./scripts/docker-compose-helper.sh restart

# Voir les logs
docker-compose -f docker-compose.complete.yml logs -f
# ou
./scripts/docker-compose-helper.sh logs

# Statut
docker-compose -f docker-compose.complete.yml ps
# ou
./scripts/docker-compose-helper.sh status
```

### Accès au conteneur

```bash
# Ouvrir un shell dans le conteneur
docker-compose -f docker-compose.complete.yml exec elidune-complete sh
# ou
./scripts/docker-compose-helper.sh shell

# Exécuter une commande dans le conteneur
docker-compose -f docker-compose.complete.yml exec elidune-complete psql -U elidune -d elidune
```

### Gestion des volumes

```bash
# Lister les volumes
docker volume ls | grep elidune

# Inspecter un volume
docker volume inspect elidune-postgres-data
docker volume inspect elidune-redis-data

# Voir l'utilisation de l'espace
docker system df -v
```

### Mise à jour de l'image

```bash
# Arrêter le service
docker-compose -f docker-compose.complete.yml stop

# Charger la nouvelle image
gunzip -c nouvelle-image.tar.gz | docker load

# Redémarrer
docker-compose -f docker-compose.complete.yml up -d
```

---

## Dépannage

### Le conteneur ne démarre pas

```bash
# Voir les logs détaillés
docker-compose -f docker-compose.complete.yml logs

# Vérifier les erreurs
docker-compose -f docker-compose.complete.yml ps
```

### PostgreSQL ne démarre pas

```bash
# Vérifier les logs PostgreSQL
docker-compose -f docker-compose.complete.yml exec elidune-complete \
    tail -f /var/log/supervisor/postgresql.err.log

# Vérifier les permissions du volume
docker volume inspect elidune-postgres-data
```

### Le serveur Elidune ne démarre pas

```bash
# Vérifier les logs du serveur
docker-compose -f docker-compose.complete.yml exec elidune-complete \
    tail -f /var/log/supervisor/elidune-server.err.log

# Vérifier la configuration
docker-compose -f docker-compose.complete.yml exec elidune-complete \
    cat /app/config/default.toml
```

### Problèmes de migrations de base de données

```bash
# Vérifier l'état des migrations
docker-compose -f docker-compose.complete.yml exec elidune-complete \
    psql -U elidune -d elidune -c "SELECT * FROM _sqlx_migrations;"

# Réinitialiser les migrations (⚠️ attention)
docker-compose -f docker-compose.complete.yml exec elidune-complete \
    psql -U elidune -d elidune -c "TRUNCATE TABLE _sqlx_migrations;"
```

### Les ports sont déjà utilisés

```bash
# Trouver quel processus utilise le port
sudo netstat -tulpn | grep :8181
sudo lsof -i :8181

# Modifier les ports dans .env
nano /opt/elidune/.env
# Changer GUI_PORT, API_PORT, etc.

# Redémarrer
docker-compose -f docker-compose.complete.yml down
docker-compose -f docker-compose.complete.yml up -d
```

### Problèmes de permissions

```bash
# Vérifier les permissions des scripts
ls -la /opt/elidune/scripts/
chmod +x /opt/elidune/scripts/*.sh
chmod +x /opt/elidune/docker/wait-and-start-server.sh
```

### Nettoyer en cas de problème

```bash
# Arrêter et supprimer le conteneur (volumes conservés)
docker-compose -f docker-compose.complete.yml down

# Supprimer aussi les volumes (⚠️ supprime les données)
docker-compose -f docker-compose.complete.yml down -v

# Nettoyer les images non utilisées
docker image prune -a
```

---

## Configuration Nginx sur le serveur hôte (optionnel)

Si vous voulez exposer Elidune via un domaine avec HTTPS :

### Exemple de configuration Nginx

```nginx
server {
    listen 80;
    server_name elidune.votre-domaine.fr;
    return 301 https://$server_name$request_uri;
}

server {
    listen 443 ssl http2;
    server_name elidune.votre-domaine.fr;

    ssl_certificate /etc/letsencrypt/live/votre-domaine.fr/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/votre-domaine.fr/privkey.pem;

    # API
    location /api {
        proxy_pass http://127.0.0.1:8282;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection 'upgrade';
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_set_header X-Forwarded-Host $host;
        proxy_set_header X-Forwarded-Port $server_port;
        proxy_cache_bypass $http_upgrade;
        proxy_connect_timeout 300s;
        proxy_send_timeout 300s;
        proxy_read_timeout 300s;
        proxy_buffering off;
    }

    # GUI
    location / {
        proxy_pass http://127.0.0.1:8181;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection 'upgrade';
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_set_header X-Forwarded-Host $host;
        proxy_set_header X-Forwarded-Port $server_port;
        proxy_cache_bypass $http_upgrade;
    }
}
```

---

## Checklist de déploiement

- [ ] Docker et Docker Compose installés sur le serveur
- [ ] Fichiers transférés sur le serveur
- [ ] Scripts rendus exécutables
- [ ] Fichier `.env` créé et configuré
- [ ] `JWT_SECRET` modifié avec une clé sécurisée
- [ ] `POSTGRES_PASSWORD` modifié (recommandé)
- [ ] Ports vérifiés et disponibles
- [ ] Image Docker chargée ou construite
- [ ] Service démarré avec `docker-compose up -d`
- [ ] Logs vérifiés pour confirmer le démarrage
- [ ] Services testés (API, GUI, PostgreSQL, Redis)
- [ ] Sauvegarde initiale effectuée

---

## Support

En cas de problème, vérifiez :
1. Les logs : `docker-compose logs -f`
2. Le statut : `docker-compose ps`
3. Les volumes : `docker volume ls`
4. L'espace disque : `df -h`

Pour plus d'informations, consultez :
- `scripts/README-docker-compose.md` - Guide détaillé Docker Compose
- Les logs du conteneur pour les erreurs spécifiques
