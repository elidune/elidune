-- Remove is_archive from specimens; use archive_date instead (archived when archive_date IS NOT NULL)
ALTER TABLE specimens DROP COLUMN IF EXISTS is_archive;
