# Elidune chat assistant

In-app chat connects a configured LLM to the **same read-only MCP tools** as `POST /api/v1/mcp`. The browser never sees API keys; the server orchestrates tool calls in-process under the user JWT (PostgreSQL RLS unchanged).

## Access

| Requirement | Detail |
|---|---|
| Right | `account_types.chat_rights` — `r` (read) minimum |
| Default | `r` for librarian/admin, `n` for guest/reader/group |
| JWT field | `rights.chatRights` |

Admins can enable chat for patrons via **Paramètres → Types de compte → Assistant chat**.

## User API (`/api/v1/chat`, JWT + `require_chat`)

| Method | Path | Description |
|---|---|---|
| GET | `/chat/providers` | Enabled LLM providers (no secrets) |
| GET | `/chat/conversations` | Current user's conversations |
| POST | `/chat/conversations` | `{ providerId, model?, title? }` |
| GET | `/chat/conversations/:id` | Conversation + messages |
| PATCH | `/chat/conversations/:id` | `{ title }` |
| DELETE | `/chat/conversations/:id` | Delete own conversation |
| POST | `/chat/conversations/:id/messages` | `{ content, providerId?, model? }` → **SSE** stream |
| POST | `/chat/conversations/:id/cancel` | Cancel in-flight generation |

SSE event types: `delta`, `tool.start`, `tool.result`, `error`, `done`.

## Admin API (`require_admin`)

| Method | Path |
|---|---|
| GET/POST | `/admin/llm/providers` |
| PUT/DELETE | `/admin/llm/providers/:id` |
| POST | `/admin/llm/providers/:id/test` |

Responses expose `apiKeySet: true/false` only — never the key.

## Configuration

```toml
[chat]
enabled = true
max_tool_rounds = 8
max_history_messages = 40
request_timeout_secs = 120

[[chat.providers]]
slug = "ollama"
label = "Ollama (local)"
kind = "openaiCompat"
base_url = "http://127.0.0.1:11434/v1"
models = ["llama3.2"]
default_model = "llama3.2"
enabled = true
```

Providers are **seeded on startup** (`INSERT … ON CONFLICT DO NOTHING`). Runtime CRUD is stored in `llm_providers`.

### Provider kinds

- **`openaiCompat`** — OpenAI, Ollama `/v1`, Groq, Mistral, Gemini ([OpenAI-compatible endpoint](https://ai.google.dev/gemini-api/docs/openai)), vLLM, LM Studio
- **`anthropic`** — Claude Messages API (`base_url` e.g. `https://api.anthropic.com/v1`)

Set `api_key` in TOML or `api_key_env` (e.g. `OPENAI_API_KEY`).

## Security notes

- Conversations are **strictly per user** (even admins cannot read another user's chats).
- MCP RLS enforces data visibility per account type.
- User messages are audit-logged as `chat.message` (truncated); LLM replies and raw SQL rows are not.

See also [README-mcp.md](README-mcp.md) for external MCP clients (Cursor).
