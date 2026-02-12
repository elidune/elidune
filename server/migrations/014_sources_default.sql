-- Add default column to sources table for default source
ALTER TABLE sources ADD COLUMN IF NOT EXISTS "default" BOOLEAN DEFAULT FALSE;

-- Ensure only one source can be marked as default
CREATE UNIQUE INDEX IF NOT EXISTS sources_default_unique ON sources ("default") WHERE "default" = true;
