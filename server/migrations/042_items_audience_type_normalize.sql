-- =============================================================================
-- Migration 042: Migrate items.audience_type from SMALLINT to VARCHAR (camelCase)
-- =============================================================================
-- New encoding convention (aligned with AudienceType in models/item.rs):
--   "general"     (was 97)
--   "juvenile"    (was 106)
--   "unknown"     (was 117)
--   NULL          left as NULL (no audience information)
-- =============================================================================

-- Step 1: add a temporary VARCHAR column
ALTER TABLE items ADD COLUMN audience_type_str VARCHAR;

-- Step 2: map integer codes to camelCase strings
UPDATE items SET audience_type_str =
    CASE audience_type
        WHEN 97  THEN 'general'
        WHEN 106 THEN 'juvenile'
        WHEN 117 THEN 'unknown'
        ELSE          'unknown'   -- any other non-NULL value → unknown
    END
WHERE audience_type IS NOT NULL;

-- Step 3: drop the old integer column and rename
ALTER TABLE items DROP COLUMN audience_type;
ALTER TABLE items RENAME COLUMN audience_type_str TO audience_type;
