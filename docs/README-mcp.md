# Elidune MCP server

Read-only [Model Context Protocol](https://modelcontextprotocol.io/) endpoint for complex SQL over the Elidune PostgreSQL database. Row-level security is enforced in the database: a language model cannot leak another patron’s personal data by writing a clever `JOIN`.

## Endpoint

| | |
|---|---|
| URL | `POST /api/v1/mcp` |
| Transport | Streamable HTTP (stateless JSON-RPC) |
| Auth | Same Bearer JWT as the REST API (`Authorization: Bearer <token>`) |
| Protocol | MCP `2025-03-26` |

`GET` / `DELETE` return 405 (no SSE session). Disable with `[mcp] enabled = false`.

## Tools

1. **`list_tables`** — tables/views the current account may `SELECT`
2. **`describe_table`** — columns for one table (`table_name` only, no schema)
3. **`query`** — a single `SELECT` / `WITH` / `EXPLAIN SELECT`

Queries run in `BEGIN` + `READ ONLY` + `statement_timeout` (default 5s). Results are capped (`max_rows`, default 200). Writes, multiple statements, and `SET ROLE` are rejected by the parser **and** would fail the read-only transaction.

## Who sees what

| Account | PostgreSQL role | `users` / `loans` / `holds` | Admin tables (`audit_log`, `email_outbox`, …) |
|---|---|---|---|
| admin | `elidune_mcp_admin` | all rows | yes (`settings.smtp_password` stripped; no Z39.50 password) |
| librarian | `elidune_mcp_staff` | all rows (including patron PII) | no |
| reader / group | `elidune_mcp_patron` | **own rows only** | no |
| guest | `elidune_mcp_catalog` | none | no (catalog / events / opening hours only) |

**Never exposed** (even to admin): `users.password`, `totp_secret`, `recovery_codes`, `recovery_codes_used`, `token_version`, `z3950servers.password`.

`SELECT * FROM users` uses the `mcp.users` view (search_path is `mcp, public`).

## Cursor example

Login via `POST /api/v1/auth/login`, then:

```json
{
  "mcpServers": {
    "elidune": {
      "url": "http://localhost:8080/api/v1/mcp",
      "headers": {
        "Authorization": "Bearer <jwt>"
      }
    }
  }
}
```

JWTs expire (`users.jwt_expiration_hours`). Re-login when the client gets 401.

## Database role

Migration `022_mcp_rls.sql` creates login role `elidune_mcp` / password `elidune_mcp` (change in production):

```sql
ALTER ROLE elidune_mcp PASSWORD 'a-strong-secret';
```

Then set `mcp.database_url` or `ELIDUNE_MCP__DATABASE_URL`.

The application user (`elidune`) still owns the tables and **bypasses RLS**, so the REST API is unchanged. MCP connections use `elidune_mcp` with `NOINHERIT` and `SET LOCAL ROLE` per request.

## Config

```toml
[mcp]
enabled = true
max_rows = 200
statement_timeout_ms = 5000
# database_url = "postgres://elidune_mcp:elidune_mcp@localhost:5432/elidune"
```

Every `query` call is written to `audit_log` as `mcp.query` (SQL truncated, no result rows).
