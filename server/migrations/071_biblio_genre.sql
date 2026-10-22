-- Restore legacy genre classification on bibliographic records (SMALLINT enum codes).
-- NULL or 0 = unknown. Used for call-number suggestions when Dewey is absent.
ALTER TABLE biblios ADD COLUMN IF NOT EXISTS genre SMALLINT;

COMMENT ON COLUMN biblios.genre IS
  'Legacy genre classification (SMALLINT codes matching Genre enum). NULL/0 = unknown.';
