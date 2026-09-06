-- Client retry / double-submit protection for desk mutations (checkout, renew).
-- Does not replace SQL uniqueness on loans/holds; stores the first completed
-- HTTP outcome for (actor_id, idempotency_key) until expires_at.

CREATE TABLE IF NOT EXISTS idempotency_keys (
    actor_id         BIGINT        NOT NULL,
    idempotency_key  VARCHAR(255)  NOT NULL,
    operation        VARCHAR(64)   NOT NULL,
    request_hash     CHAR(64)      NOT NULL,
    status_code      SMALLINT,
    response_body    JSONB,
    created_at       TIMESTAMPTZ   NOT NULL DEFAULT NOW(),
    expires_at       TIMESTAMPTZ   NOT NULL,
    PRIMARY KEY (actor_id, idempotency_key)
);

CREATE INDEX IF NOT EXISTS idx_idempotency_keys_expires_at
    ON idempotency_keys (expires_at);

COMMENT ON TABLE idempotency_keys IS
    'Replay store for Idempotency-Key: same actor+key+payload returns the stored response within TTL.';
COMMENT ON COLUMN idempotency_keys.request_hash IS
    'SHA-256 hex of the canonical request fingerprint (operation + payload).';
COMMENT ON COLUMN idempotency_keys.status_code IS
    'NULL while the first request is in flight; set when a 2xx body is stored.';
