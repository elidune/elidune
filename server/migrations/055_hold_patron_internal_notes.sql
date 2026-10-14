-- Split the hold note (#137).
-- `holds.notes` was accepted on POST /holds from anyone who can place a hold:
-- a patron on their own hold, and staff (`holds_rights` write) when placing a
-- hold for a patron. The desk labeled the field "Visible to the patron", but
-- staff could store an internal remark in the same column. It was not
-- exclusively patron-written, so existing values move to internal_note (same
-- rule as items in #227). patron_note starts NULL.
-- Safe to re-run: new columns are added only if missing, the copy runs only
-- while `notes` still exists, and a row that already has internal_note is
-- left alone.

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
        SET internal_note = notes
        WHERE notes IS NOT NULL
          AND btrim(notes) <> ''
          AND internal_note IS NULL;

        ALTER TABLE holds DROP COLUMN notes;
    END IF;
END $$;

COMMENT ON COLUMN holds.patron_note IS
    'Note visible to the patron and to staff. Former holds.notes values are not copied here.';
COMMENT ON COLUMN holds.internal_note IS
    'Staff-only hold note. Former holds.notes values live here.';
