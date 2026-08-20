-- Chat assistant: account rights, LLM providers, conversations and messages.

ALTER TABLE account_types
    ADD COLUMN IF NOT EXISTS chat_rights VARCHAR(1);

UPDATE account_types
SET chat_rights = 'r'
WHERE code IN ('librarian', 'admin');

UPDATE account_types
SET chat_rights = 'n'
WHERE chat_rights IS NULL;

COMMENT ON COLUMN account_types.chat_rights IS 'n/r: none or read access to /chat assistant';

CREATE TABLE IF NOT EXISTS llm_providers (
    id BIGINT PRIMARY KEY,
    slug VARCHAR(64) NOT NULL UNIQUE,
    label VARCHAR(128) NOT NULL,
    kind VARCHAR(32) NOT NULL,
    base_url TEXT NOT NULL,
    api_key TEXT,
    api_key_env VARCHAR(128),
    models JSONB NOT NULL DEFAULT '[]'::jsonb,
    default_model VARCHAR(128),
    enabled BOOLEAN NOT NULL DEFAULT true,
    sort_order INT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_llm_providers_enabled ON llm_providers (enabled, sort_order);

CREATE TABLE IF NOT EXISTS chat_conversations (
    id BIGINT PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    title VARCHAR(256) NOT NULL DEFAULT 'New conversation',
    provider_id BIGINT NOT NULL REFERENCES llm_providers (id),
    model VARCHAR(128) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_chat_conversations_user ON chat_conversations (user_id, updated_at DESC);

CREATE TABLE IF NOT EXISTS chat_messages (
    id BIGINT PRIMARY KEY,
    conversation_id BIGINT NOT NULL REFERENCES chat_conversations (id) ON DELETE CASCADE,
    role VARCHAR(16) NOT NULL,
    content TEXT,
    tool_name VARCHAR(64),
    tool_call_id VARCHAR(128),
    tool_arguments JSONB,
    tool_result JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_chat_messages_conversation ON chat_messages (conversation_id, created_at);
