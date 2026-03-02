-- =============================================================================
-- Migration 020: MARC Schema Refactoring
-- =============================================================================
-- Purpose: Refactor items, specimens, collections, editions, and series tables
-- to align with MARC data model. Consolidates title/author fields, renames
-- columns for clarity, adds search_vector, timestamps, and indexes.
-- =============================================================================

-- =============================================================================
-- 0. DROP LEGACY search_vector TRIGGER AND FUNCTION
-- =============================================================================

DROP TRIGGER IF EXISTS items_search_vector_trigger ON items;
DROP FUNCTION IF EXISTS items_search_vector_update() CASCADE;

-- =============================================================================
-- 1. ADD NEW COLUMNS TO items
-- =============================================================================

ALTER TABLE items ADD COLUMN IF NOT EXISTS marc_data JSONB;
ALTER TABLE items ADD COLUMN IF NOT EXISTS call_number VARCHAR;
ALTER TABLE items ADD COLUMN IF NOT EXISTS search_vector tsvector;
ALTER TABLE items ADD COLUMN IF NOT EXISTS author_id INTEGER;
ALTER TABLE items ADD COLUMN IF NOT EXISTS author_role VARCHAR;

-- =============================================================================
-- 2. TITLE CONSOLIDATION ON items
-- =============================================================================

ALTER TABLE items RENAME COLUMN title1 TO title;
ALTER TABLE items DROP COLUMN IF EXISTS title2;
ALTER TABLE items DROP COLUMN IF EXISTS title3;
ALTER TABLE items DROP COLUMN IF EXISTS title4;

-- =============================================================================
-- 3. AUTHOR CONSOLIDATION ON items
-- =============================================================================

UPDATE items SET author_id = author1_ids[1]
WHERE author1_ids IS NOT NULL AND array_length(author1_ids, 1) > 0;

UPDATE items SET author_role = split_part(author1_functions, ',', 1)
WHERE author1_functions IS NOT NULL AND author1_functions != '';

UPDATE items SET call_number = dewey
WHERE call_number IS NULL AND dewey IS NOT NULL;

ALTER TABLE items DROP COLUMN IF EXISTS dewey;

ALTER TABLE items DROP COLUMN IF EXISTS author1_ids;
ALTER TABLE items DROP COLUMN IF EXISTS author1_functions;
ALTER TABLE items DROP COLUMN IF EXISTS author2_ids;
ALTER TABLE items DROP COLUMN IF EXISTS author2_functions;
ALTER TABLE items DROP COLUMN IF EXISTS author3_ids;
ALTER TABLE items DROP COLUMN IF EXISTS author3_functions;

-- =============================================================================
-- 4. RENAME COLUMNS ON items
-- =============================================================================

ALTER TABLE items RENAME COLUMN crea_date TO created_at;
ALTER TABLE items RENAME COLUMN modif_date TO updated_at;
ALTER TABLE items RENAME COLUMN archived_date TO archived_at;
ALTER TABLE items RENAME COLUMN serie_id TO series_id;
ALTER TABLE items RENAME COLUMN serie_vol_number TO series_volume_number;
ALTER TABLE items RENAME COLUMN collection_number_sub TO collection_sequence_number;
ALTER TABLE items RENAME COLUMN collection_vol_number TO collection_volume_number;
-- Migrate is_archive data into archived_at before dropping
UPDATE items SET archived_at = NOW() WHERE (is_archive = 1 OR is_archive IS NOT NULL) AND is_archive != 0 AND archived_at IS NULL;
ALTER TABLE items DROP COLUMN IF EXISTS is_archive;
ALTER TABLE items RENAME COLUMN edition_date TO edition_statement_date;
ALTER TABLE items RENAME COLUMN nb_pages TO page_extent;
ALTER TABLE items RENAME COLUMN public_type TO audience_type;
ALTER TABLE items RENAME COLUMN content TO table_of_contents;
ALTER TABLE items RENAME COLUMN addon TO accompanying_material;

-- =============================================================================
-- 5. RENAME COLUMNS ON specimens
-- =============================================================================

ALTER TABLE specimens RENAME COLUMN id_item TO item_id;
ALTER TABLE specimens RENAME COLUMN crea_date TO created_at;
ALTER TABLE specimens RENAME COLUMN modif_date TO updated_at;
ALTER TABLE specimens RENAME COLUMN archive_date TO archived_at;
ALTER TABLE specimens RENAME COLUMN status TO borrow_status;
ALTER TABLE specimens RENAME COLUMN codestat TO circulation_status;

-- =============================================================================
-- 6. RENAME COLUMNS ON collections
-- =============================================================================

ALTER TABLE collections RENAME COLUMN title1 TO primary_title;
ALTER TABLE collections RENAME COLUMN title2 TO secondary_title;
ALTER TABLE collections RENAME COLUMN title3 TO tertiary_title;

-- =============================================================================
-- 7. RENAME COLUMNS ON editions
-- =============================================================================

ALTER TABLE editions RENAME COLUMN name TO publisher_name;
ALTER TABLE editions RENAME COLUMN place TO place_of_publication;

-- =============================================================================
-- 8. ADD TIMESTAMPS TO series
-- =============================================================================

ALTER TABLE series ADD COLUMN IF NOT EXISTS created_at TIMESTAMPTZ DEFAULT NOW();
ALTER TABLE series ADD COLUMN IF NOT EXISTS updated_at TIMESTAMPTZ DEFAULT NOW();

-- =============================================================================
-- 9. ADD TIMESTAMPS TO collections
-- =============================================================================

ALTER TABLE collections ADD COLUMN IF NOT EXISTS created_at TIMESTAMPTZ DEFAULT NOW();
ALTER TABLE collections ADD COLUMN IF NOT EXISTS updated_at TIMESTAMPTZ DEFAULT NOW();

-- =============================================================================
-- 10. ADD FK CONSTRAINT FOR author_id
-- =============================================================================

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'fk_items_author'
    ) THEN
        ALTER TABLE items
            ADD CONSTRAINT fk_items_author FOREIGN KEY (author_id)
            REFERENCES authors(id) ON DELETE SET NULL;
    END IF;
END $$;

-- =============================================================================
-- 11. ADD INDEXES
-- =============================================================================

CREATE INDEX IF NOT EXISTS idx_items_search_vector ON items USING GIN (search_vector);
CREATE INDEX IF NOT EXISTS idx_items_author_id ON items (author_id);
CREATE INDEX IF NOT EXISTS idx_items_call_number ON items (call_number);

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_indexes WHERE indexname = 'idx_series_key_unique'
    ) THEN
        CREATE UNIQUE INDEX idx_series_key_unique ON series (key) WHERE key IS NOT NULL;
    END IF;
END $$;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_indexes WHERE indexname = 'idx_collections_key_unique'
    ) THEN
        CREATE UNIQUE INDEX idx_collections_key_unique ON collections (key) WHERE key IS NOT NULL;
    END IF;
END $$;

-- =============================================================================
-- 12. FK CONSTRAINT fk_items_serie
-- =============================================================================
-- The existing FK fk_items_serie references serie_id. After rename to series_id,
-- PostgreSQL keeps the constraint valid (it references the column by OID, not name).
-- No action needed for the constraint to work.
-- =============================================================================
