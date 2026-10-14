-- Split the physical-copy note (#109).
-- Existing `notes` are staff remarks (condition, repair, provenance) and move to internal_note.
-- public_note is the only copy note returned to patrons and the OPAC.

ALTER TABLE items ADD COLUMN IF NOT EXISTS public_note TEXT;
ALTER TABLE items ADD COLUMN IF NOT EXISTS internal_note TEXT;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM information_schema.columns
        WHERE table_schema = 'public'
          AND table_name = 'items'
          AND column_name = 'notes'
    ) THEN
        UPDATE items
        SET internal_note = notes
        WHERE notes IS NOT NULL
          AND btrim(notes) <> ''
          AND internal_note IS NULL;

        ALTER TABLE items DROP COLUMN notes;
    END IF;
END $$;

COMMENT ON COLUMN items.public_note IS
    'Copy note shown to patrons and the OPAC (for example "CD missing").';
COMMENT ON COLUMN items.internal_note IS
    'Staff-only copy note. Former items.notes values live here.';
