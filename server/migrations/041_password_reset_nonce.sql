-- Password-reset links are single-use and superseded by a newer link.
-- The JWT carries this counter. It moves when a link is issued, when a reset
-- succeeds, and when the password changes (including a profile password change).

ALTER TABLE users
    ADD COLUMN IF NOT EXISTS password_reset_nonce BIGINT NOT NULL DEFAULT 0;

COMMENT ON COLUMN users.password_reset_nonce IS
    'Invalidates outstanding password-reset links. Incremented when a link is issued, when a reset succeeds, and when the password changes.';
