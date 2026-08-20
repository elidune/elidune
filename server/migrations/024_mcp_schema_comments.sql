-- Domain comments for MCP describe_table / schema_overview (Elidune LMS vocabulary).

COMMENT ON TABLE loans IS 'Active circulation loans (emprunts en cours). Filter returned_at IS NULL for open loans.';
COMMENT ON COLUMN loans.user_id IS 'Borrower (users.id). RLS: patrons see only their rows.';
COMMENT ON COLUMN loans.item_id IS 'Borrowed physical copy (items.id). Join items → biblios for title.';
COMMENT ON COLUMN loans.date IS 'Loan start timestamp.';
COMMENT ON COLUMN loans.expiry_at IS 'Due date / return-by date.';
COMMENT ON COLUMN loans.returned_at IS 'When the copy was returned; NULL while loan is active.';

COMMENT ON TABLE holds IS 'Reservation queue entries (réservations). Status: pending, ready, cancelled, expired.';
COMMENT ON COLUMN holds.user_id IS 'Patron who placed the hold.';
COMMENT ON COLUMN holds.item_id IS 'Physical copy reserved (items.id).';

COMMENT ON TABLE items IS 'Physical copies / specimens (exemplaires) of a bibliographic record.';
COMMENT ON COLUMN items.biblio_id IS 'Parent bibliographic record (biblios.id).';
COMMENT ON COLUMN items.barcode IS 'Copy barcode for circulation.';

COMMENT ON TABLE biblios IS 'Bibliographic records (notices): title, ISBN, media type, etc.';
COMMENT ON COLUMN biblios.title IS 'Main title of the work.';

COMMENT ON TABLE loans_archives IS 'Returned / archived loan history.';

COMMENT ON VIEW mcp.users IS 'Safe user projection for MCP (no password, TOTP, or token_version).';
