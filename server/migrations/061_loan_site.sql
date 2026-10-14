-- Record the site where a loan was made (#128).
--
-- loans.site_id is the desk site at checkout (the staff member's current site).
-- It is nullable. Loans that already exist stay NULL: this migration does not
-- backfill a site. Calendar (due date, renewal, fines) and loan statistics
-- resolve a NULL site_id to the library default site at query time
-- (sources."default" = true, the same row sources_get_default returns).
-- They do not use items.source_id.
--
-- loans_archives.site_id is copied from the loan when it is closed so a returned
-- loan still counts at the site where it was made. Existing archive rows stay
-- NULL and resolve to that same default site. Not backfilled.
-- Deleting a source referenced by an archive is refused (ON DELETE RESTRICT).
-- An active loan's site is cleared instead (ON DELETE SET NULL) and then
-- follows the default site.

ALTER TABLE loans
    ADD COLUMN site_id BIGINT REFERENCES sources (id) ON DELETE SET NULL;

COMMENT ON COLUMN loans.site_id IS
    'Site where the loan was made. NULL when the loan predates this column or the request omitted the desk; calendar and statistics then use the library default site (sources."default"), not items.source_id.';

CREATE INDEX IF NOT EXISTS idx_loans_site_id ON loans (site_id);

ALTER TABLE loans_archives
    ADD COLUMN site_id BIGINT REFERENCES sources (id) ON DELETE RESTRICT;

COMMENT ON COLUMN loans_archives.site_id IS
    'Copied from loans.site_id when the loan is archived. NULL when that loan had no recorded site; readers then use the library default site. Not backfilled. ON DELETE RESTRICT.';

CREATE INDEX IF NOT EXISTS idx_loans_archives_site_id ON loans_archives (site_id);
