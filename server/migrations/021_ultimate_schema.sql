-- =============================================================================
-- Migration 021: Ultimate MARC-aligned Schema
-- =============================================================================
-- Purpose: Multi-author support via junction table, MARC format tracking,
-- simplified lifecycle (archived_at as sole soft-delete indicator),
-- volume designation on specimens, edition date consolidation, proper indexes.
-- =============================================================================

-- =============================================================================
-- 0. DROP search_vector TRIGGER, FUNCTION, INDEX AND COLUMN
-- =============================================================================

DROP TRIGGER IF EXISTS items_search_vector_trigger ON items;
DROP FUNCTION IF EXISTS items_search_vector_update() CASCADE;
DROP INDEX IF EXISTS idx_items_search_vector;
ALTER TABLE items DROP COLUMN IF EXISTS search_vector;

-- =============================================================================
-- 1. CREATE item_authors JUNCTION TABLE (N:M items <-> authors)
-- =============================================================================

CREATE TABLE IF NOT EXISTS item_authors (
    id SERIAL PRIMARY KEY,
    item_id INTEGER NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    author_id INTEGER NOT NULL REFERENCES authors(id) ON DELETE CASCADE,
    role VARCHAR(10),
    author_type SMALLINT NOT NULL DEFAULT 0,
    position SMALLINT NOT NULL DEFAULT 1,
    UNIQUE(item_id, author_id, role)
);

-- =============================================================================
-- 2. MIGRATE EXISTING author_id / author_role DATA INTO item_authors
-- =============================================================================

INSERT INTO item_authors (item_id, author_id, role, author_type, position)
SELECT id, author_id, author_role, 0, 1
FROM items
WHERE author_id IS NOT NULL
ON CONFLICT DO NOTHING;

-- =============================================================================
-- 3. DROP OLD AUTHOR COLUMNS FROM items
-- =============================================================================

ALTER TABLE items DROP COLUMN IF EXISTS author_id;
ALTER TABLE items DROP COLUMN IF EXISTS author_role;

-- Drop the FK constraint that referenced author_id
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_items_author') THEN
        ALTER TABLE items DROP CONSTRAINT fk_items_author;
    END IF;
END $$;

-- =============================================================================
-- 4. ADD marc_format COLUMN
-- =============================================================================

ALTER TABLE items ADD COLUMN IF NOT EXISTS marc_format VARCHAR(10);

-- =============================================================================
-- 5. RENAME marc_data -> marc_record
-- =============================================================================

DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'items' AND column_name = 'marc_data'
    ) THEN
        ALTER TABLE items RENAME COLUMN marc_data TO marc_record;
    END IF;
END $$;

-- =============================================================================
-- 6. SIMPLIFY LIFECYCLE: archived_at replaces lifecycle_status for deletion
-- =============================================================================

-- Items: ensure all lifecycle_status=2 rows have archived_at set
UPDATE items SET archived_at = COALESCE(archived_at, NOW())
WHERE lifecycle_status = 2 AND archived_at IS NULL;

-- Rename lifecycle_status to status (0=active, 1=unavailable)
ALTER TABLE items RENAME COLUMN lifecycle_status TO status;
UPDATE items SET status = 0 WHERE status = 2;

-- Specimens: ensure all lifecycle_status=2 rows have archived_at set
UPDATE specimens SET archived_at = COALESCE(archived_at, NOW())
WHERE lifecycle_status = 2 AND archived_at IS NULL;

-- Drop lifecycle_status from specimens (archived_at is sufficient)
ALTER TABLE specimens DROP COLUMN IF EXISTS lifecycle_status;

-- =============================================================================
-- 7. CONSOLIDATE EDITION DATE INTO editions TABLE
-- =============================================================================

ALTER TABLE editions ADD COLUMN IF NOT EXISTS date VARCHAR;
ALTER TABLE editions ADD COLUMN IF NOT EXISTS created_at TIMESTAMPTZ DEFAULT NOW();
ALTER TABLE editions ADD COLUMN IF NOT EXISTS updated_at TIMESTAMPTZ DEFAULT NOW();

UPDATE editions e SET date = sub.edition_statement_date
FROM (
    SELECT DISTINCT ON (edition_id) edition_id, edition_statement_date
    FROM items
    WHERE edition_id IS NOT NULL AND edition_statement_date IS NOT NULL
    ORDER BY edition_id, updated_at DESC
) sub
WHERE e.id = sub.edition_id AND e.date IS NULL;

ALTER TABLE items DROP COLUMN IF EXISTS edition_statement_date;



-- =============================================================================
-- 9. ADD volume_designation TO specimens
-- =============================================================================

ALTER TABLE specimens ADD COLUMN IF NOT EXISTS volume_designation VARCHAR(50);

-- =============================================================================
-- 10. INDEXES FOR PERFORMANCE AND NAVIGATION
-- =============================================================================

CREATE INDEX IF NOT EXISTS idx_item_authors_item ON item_authors(item_id);
CREATE INDEX IF NOT EXISTS idx_item_authors_author ON item_authors(author_id);
CREATE INDEX IF NOT EXISTS idx_items_series_vol ON items(series_id, series_volume_number);
CREATE INDEX IF NOT EXISTS idx_items_collection_vol ON items(collection_id, collection_volume_number);
CREATE INDEX IF NOT EXISTS idx_items_isbn ON items(isbn) WHERE isbn IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_items_media_type ON items(media_type);
CREATE INDEX IF NOT EXISTS idx_items_active ON items(archived_at) WHERE archived_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_specimens_item ON specimens(item_id);
CREATE INDEX IF NOT EXISTS idx_specimens_active ON specimens(archived_at) WHERE archived_at IS NULL;
