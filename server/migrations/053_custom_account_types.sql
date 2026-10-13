-- Custom account types (#201).
-- Built-in rows become system types (not deletable). Users keep their codes.
-- users.account_type stays ON DELETE RESTRICT so an assigned type cannot disappear.

ALTER TABLE account_types
    ADD COLUMN IF NOT EXISTS is_system BOOLEAN NOT NULL DEFAULT FALSE;

UPDATE account_types
SET is_system = TRUE
WHERE code IN ('guest', 'reader', 'librarian', 'admin', 'group');

COMMENT ON COLUMN account_types.is_system IS
    'Built-in account types cannot be deleted. Custom types are false.';

DO $$
DECLARE
    cons text;
BEGIN
    FOR cons IN
        SELECT c.conname
        FROM pg_constraint c
        JOIN pg_class t ON t.oid = c.conrelid
        JOIN pg_namespace n ON n.oid = t.relnamespace
        JOIN pg_attribute a ON a.attrelid = t.oid AND a.attnum = ANY (c.conkey)
        WHERE n.nspname = 'public'
          AND t.relname = 'users'
          AND a.attname = 'account_type'
          AND c.contype = 'f'
    LOOP
        EXECUTE format('ALTER TABLE users DROP CONSTRAINT %I', cons);
    END LOOP;
END $$;

ALTER TABLE users
    ADD CONSTRAINT users_account_type_fkey
    FOREIGN KEY (account_type)
    REFERENCES account_types (code)
    ON DELETE RESTRICT;
