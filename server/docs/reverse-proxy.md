# Reverse proxy (Nginx and Apache)

Place Elidune Server behind a reverse proxy when you need TLS termination, a single hostname for the API and a frontend, or standard HTTP ports (80/443).

The application listens on HTTP only (`[server]` in `config/default.toml`). The proxy should forward:

- `Host`, `X-Forwarded-*`, and `X-Forwarded-Proto` so the API sees the original scheme and host if you ever add strict URL checks.
- Long timeouts for large imports or slow clients (optional but recommended).

Default API base path prefix: `/api` (e.g. `/api/v1/...`). Swagger UI is under `/swagger-ui` on the same server port.

---

## Nginx

### API only (single upstream)

Replace `127.0.0.1:8080` with your Elidune bind address and port.

```nginx
upstream elidune_api {
    server 127.0.0.1:8080;
    keepalive 32;
}

server {
    listen 443 ssl http2;
    server_name elidune.example.org;

    ssl_certificate     /etc/letsencrypt/live/elidune.example.org/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/elidune.example.org/privkey.pem;

    location / {
        proxy_pass http://elidune_api;
        proxy_http_version 1.1;
        proxy_set_header Host              $host;
        proxy_set_header X-Real-IP         $remote_addr;
        proxy_set_header X-Forwarded-For   $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_set_header X-Forwarded-Host  $host;
        proxy_set_header X-Forwarded-Port  $server_port;
        proxy_connect_timeout 300s;
        proxy_send_timeout    300s;
        proxy_read_timeout    300s;
        proxy_buffering off;
    }
}
```

HTTP → HTTPS redirect (port 80):

```nginx
server {
    listen 80;
    server_name elidune.example.org;
    return 301 https://$host$request_uri;
}
```

### Split API and static frontend (two upstreams)

If the UI is served separately (e.g. another port or container), use two `location` blocks:

```nginx
upstream elidune_api  { server 127.0.0.1:8080; }
upstream elidune_gui  { server 127.0.0.1:8181; }

server {
    listen 443 ssl http2;
    server_name elidune.example.org;
    # ... ssl_certificate directives ...

    location /api {
        proxy_pass http://elidune_api;
        proxy_http_version 1.1;
        proxy_set_header Host              $host;
        proxy_set_header X-Real-IP         $remote_addr;
        proxy_set_header X-Forwarded-For   $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_set_header X-Forwarded-Host  $host;
        proxy_set_header X-Forwarded-Port  $server_port;
        proxy_connect_timeout 300s;
        proxy_send_timeout    300s;
        proxy_read_timeout    300s;
        proxy_buffering off;
    }

    location / {
        proxy_pass http://elidune_gui;
        proxy_http_version 1.1;
        proxy_set_header Host              $host;
        proxy_set_header X-Real-IP         $remote_addr;
        proxy_set_header X-Forwarded-For   $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
```

Reload Nginx after testing: `sudo nginx -t && sudo systemctl reload nginx`.

---

## Apache HTTP Server (2.4+)

Enable modules:

```bash
sudo a2enmod proxy proxy_http headers ssl
sudo systemctl reload apache2
```

### API only (VirtualHost)

Replace host, port, and certificate paths.

```apache
<VirtualHost *:443>
    ServerName elidune.example.org

    SSLEngine on
    SSLCertificateFile      /etc/letsencrypt/live/elidune.example.org/fullchain.pem
    SSLCertificateKeyFile   /etc/letsencrypt/live/elidune.example.org/privkey.pem

    ProxyPreserveHost On
    RequestHeader set X-Forwarded-Proto "https"
    RequestHeader set X-Forwarded-Port "443"

    ProxyPass        / http://127.0.0.1:8080/
    ProxyPassReverse / http://127.0.0.1:8080/

    Timeout 300
</VirtualHost>
```

Port 80 redirect:

```apache
<VirtualHost *:80>
    ServerName elidune.example.org
    Redirect permanent / https://elidune.example.org/
</VirtualHost>
```

### Split API and GUI

Use `Location` with different backends:

```apache
<VirtualHost *:443>
    ServerName elidune.example.org
    SSLEngine on
    SSLCertificateFile      /etc/letsencrypt/live/elidune.example.org/fullchain.pem
    SSLCertificateKeyFile   /etc/letsencrypt/live/elidune.example.org/privkey.pem

    ProxyPreserveHost On
    RequestHeader set X-Forwarded-Proto "https"

    <Location /api>
        ProxyPass http://127.0.0.1:8080/api
        ProxyPassReverse http://127.0.0.1:8080/api
    </Location>

    <Location />
        ProxyPass http://127.0.0.1:8181/
        ProxyPassReverse http://127.0.0.1:8181/
    </Location>

    Timeout 300
</VirtualHost>
```

Reload: `sudo apachectl configtest && sudo systemctl reload apache2`.

---

## Notes

- **CORS:** If the browser calls the API on another origin, set `[server].cors_origins` in TOML to your frontend origin(s) for production.
- **WebSockets:** Elidune’s API is primarily HTTP/JSON; if you add WebSocket routes later, use `Upgrade` and `Connection` headers in Nginx (see Nginx WebSocket proxy docs).
- **“Complete” Docker stack** (API + UI + DB in one compose): see [README-docker.md](../README-docker.md) for host-level Nginx in front of published ports.
