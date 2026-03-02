-- Remove redundant item_id from loans and loans_archives.
-- Item is always derivable via specimen_id -> specimens.item_id.
ALTER TABLE loans DROP COLUMN IF EXISTS item_id;
ALTER TABLE loans_archives DROP COLUMN IF EXISTS item_id;
