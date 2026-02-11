-- Add archive support to sources table
ALTER TABLE sources ADD COLUMN IF NOT EXISTS is_archive SMALLINT DEFAULT 0;
ALTER TABLE sources ADD COLUMN IF NOT EXISTS archive_date TIMESTAMPTZ;
