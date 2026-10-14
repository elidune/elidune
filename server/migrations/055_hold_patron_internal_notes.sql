-- Split the hold note (#137).
-- The single `notes` column is the patron-visible remark (staff label it
-- "Visible to the patron"). Those values move to patron_note.
-- internal_note is staff-only and starts empty.

ALTER TABLE holds ADD COLUMN IF NOT EXISTS patron_note TEXT;
ALTER TABLE holds ADD COLUMN IF NOT EXISTS internal_note TEXT;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM information_schema.columns
        WHERE table_schema = 'public'
          AND table_name = 'holds'
          AND column_name = 'notes'
    ) THEN
        UPDATE holds
        SET patron_note = notes
        WHERE notes IS NOT NULL
          AND btrim(notes) <> ''
          AND patron_note IS NULL;

        ALTER TABLE holds DROP COLUMN notes;
    END IF;
END $$;

COMMENT ON COLUMN holds.patron_note IS
    'Note visible to the patron and to staff.';
COMMENT ON COLUMN holds.internal_note IS
    'Staff-only hold note. Never returned on patron or public hold views.';
