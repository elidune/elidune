-- Remove nb_specimens column from items table
-- All specimen counts should now be computed via JOIN with specimens table
ALTER TABLE items DROP COLUMN IF EXISTS nb_specimens;
