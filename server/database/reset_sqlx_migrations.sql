-- =============================================================================
-- One-shot: align _sqlx_migrations after the 001 squash (former 002–072).
-- =============================================================================
-- Run ONCE on each existing database that was already at migration 072, BEFORE
-- deploying a server build that only ships migrations/001_initial_schema.sql.
--
--   psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -f database/reset_sqlx_migrations.sql
--
-- Fresh databases: do not run this; use `sqlx migrate run` instead.
--
-- Checksum = SHA-384 of migrations/001_initial_schema.sql (sqlx-cli convention).
-- If you edit 001, recompute and update the decode(...) value below:
--   python3 -c "import hashlib,pathlib; print(hashlib.sha384(pathlib.Path('migrations/001_initial_schema.sql').read_bytes()).hexdigest())"
-- =============================================================================

BEGIN;

CREATE TABLE IF NOT EXISTS _sqlx_migrations (
    version        BIGINT PRIMARY KEY,
    description    TEXT NOT NULL,
    installed_on   TIMESTAMPTZ NOT NULL DEFAULT now(),
    success        BOOLEAN NOT NULL,
    checksum       BYTEA NOT NULL,
    execution_time BIGINT NOT NULL
);

DELETE FROM _sqlx_migrations;

INSERT INTO _sqlx_migrations (version, description, installed_on, success, checksum, execution_time)
VALUES (
    1,
    'initial schema',
    now(),
    true,
    decode(
        '33c48c2dd6836326adce86b943c0bc806fe41a3683d885b4f7a6d9c446b55886d15c813e48b498da2d09550f7b4c6ad5',
        'hex'
    ),
    0
);

COMMIT;
