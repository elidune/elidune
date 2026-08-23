-- Additional MCP schema comments (biblios text fields, loan history).

COMMENT ON COLUMN biblios.abstract IS 'Summary/resume of the work. Column name is abstract (NOT summary). Quote in SQL: b.abstract or b."abstract".';
COMMENT ON COLUMN biblios.subject IS 'Subject headings / topics for discovery and recommendations.';
COMMENT ON COLUMN biblios.keywords IS 'Keyword array (text[]) for catalog search.';
COMMENT ON COLUMN biblios.media_type IS 'Material type slug (book, ebook, …).';
COMMENT ON COLUMN biblios.isbn IS 'Normalized ISBN when known.';

COMMENT ON COLUMN loans_archives.user_id IS 'Borrower (users.id). RLS: patrons see only their rows.';
COMMENT ON COLUMN loans_archives.item_id IS 'Returned copy (items.id). Join items → biblios for title/abstract.';
COMMENT ON COLUMN loans_archives.date IS 'Original loan start timestamp.';
COMMENT ON COLUMN loans_archives.returned_at IS 'When the copy was returned to the library.';
