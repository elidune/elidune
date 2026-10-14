-- Record the site where a loan was made (#128).
--
-- loans.site_id is the desk site at checkout (the staff member's current site).
-- It is nullable. Loans that already exist stay NULL: this migration does not
-- guess a backfill from the item home site. Calendar (due date, renewal, fines)
-- and loan statistics use loans.site_id when it is set, and items.source_id
-- when it is not.
--
-- loans_archives.site_id is copied from the loan when it is closed so a returned
-- loan still counts at the site where it was made. Existing archive rows stay
-- NULL and follow the same item-home fallback. Not backfilled.

ALTER TABLE loans
    ADD COLUMN site_id BIGINT REFERENCES sources (id) ON DELETE SET NULL;

COMMENT ON COLUMN loans.site_id IS
    'Site where the loan was made. NULL when the loan predates this column; calendar and statistics then use items.source_id.';

CREATE INDEX IF NOT EXISTS idx_loans_site_id ON loans (site_id);

ALTER TABLE loans_archives
    ADD COLUMN site_id BIGINT REFERENCES sources (id) ON DELETE SET NULL;

COMMENT ON COLUMN loans_archives.site_id IS
    'Copied from loans.site_id when the loan is archived. NULL when that loan had no recorded site. Not backfilled.';

CREATE INDEX IF NOT EXISTS idx_loans_archives_site_id ON loans_archives (site_id);
