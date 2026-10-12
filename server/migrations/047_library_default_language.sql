-- Default public UI language for the library (« Ma bibliothèque »).
-- NULL means unset. Clients fall back to French only when this is null.
-- Allowed codes match the public UI: fr, en, de, es.

ALTER TABLE library_info
    ADD COLUMN IF NOT EXISTS language TEXT;

ALTER TABLE library_info
    DROP CONSTRAINT IF EXISTS library_info_language_check;

ALTER TABLE library_info
    ADD CONSTRAINT library_info_language_check
    CHECK (language IS NULL OR language IN ('fr', 'en', 'de', 'es'));

COMMENT ON COLUMN library_info.language IS
    'Default public UI language: fr, en, de, or es. NULL means unset (clients fall back to fr).';
