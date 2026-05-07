-- Per-account-type rights for cultural events API (GET vs POST/PUT/DELETE/announcements).

ALTER TABLE account_types
    ADD COLUMN IF NOT EXISTS events_rights VARCHAR(1);

UPDATE account_types
SET events_rights = 'w'
WHERE code IN ('librarian', 'admin');

UPDATE account_types
SET events_rights = 'n'
WHERE events_rights IS NULL;

COMMENT ON COLUMN account_types.events_rights IS 'n/r/w: none, read, or write access to /events';
