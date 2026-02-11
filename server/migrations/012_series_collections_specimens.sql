-- =============================================================================
-- Migration 012: Series, Collections, and Specimens Refactoring
-- =============================================================================
-- This migration adds missing fields, constraints, and fixes schema issues
-- for series, collections, and specimens tables.
-- =============================================================================

-- =============================================
-- SERIES IMPROVEMENTS
-- =============================================

-- Add ISSN column to series table
ALTER TABLE series ADD COLUMN IF NOT EXISTS issn VARCHAR;

-- =============================================
-- SPECIMENS IMPROVEMENTS
-- =============================================

-- Add UNIQUE constraint on specimen identification (barcode)
-- First, check for and handle duplicates if any exist
DO $$
BEGIN
    -- Check if constraint already exists
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'specimens_identification_unique'
    ) THEN
        -- Check for duplicates before adding constraint
        IF EXISTS (
            SELECT identification, COUNT(*) 
            FROM specimens 
            WHERE identification IS NOT NULL 
            GROUP BY identification 
            HAVING COUNT(*) > 1
        ) THEN
            RAISE NOTICE 'Warning: Duplicate identifications found in specimens table. Constraint not added.';
            RAISE NOTICE 'Please clean up duplicates before running this migration again.';
        ELSE
            ALTER TABLE specimens ADD CONSTRAINT specimens_identification_unique UNIQUE (identification);
        END IF;
    END IF;
END $$;

-- Add FK constraints for specimens
DO $$
BEGIN
    -- Add FK constraint for specimens -> items
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'fk_specimens_item'
    ) THEN
        ALTER TABLE specimens
            ADD CONSTRAINT fk_specimens_item FOREIGN KEY (id_item) 
            REFERENCES items(id) ON DELETE CASCADE;
    END IF;

    -- Add FK constraint for specimens -> sources
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'fk_specimens_source'
    ) THEN
        ALTER TABLE specimens
            ADD CONSTRAINT fk_specimens_source FOREIGN KEY (source_id) 
            REFERENCES sources(id) ON DELETE SET NULL;
    END IF;
END $$;

-- =============================================
-- ITEMS FK CONSTRAINTS
-- =============================================

-- Add FK constraints for items -> series/collections/editions
DO $$
BEGIN
    -- Add FK constraint for items -> series
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'fk_items_serie'
    ) THEN
        ALTER TABLE items
            ADD CONSTRAINT fk_items_serie FOREIGN KEY (serie_id) 
            REFERENCES series(id) ON DELETE SET NULL;
    END IF;

    -- Add FK constraint for items -> collections
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'fk_items_collection'
    ) THEN
        ALTER TABLE items
            ADD CONSTRAINT fk_items_collection FOREIGN KEY (collection_id) 
            REFERENCES collections(id) ON DELETE SET NULL;
    END IF;

    -- Add FK constraint for items -> editions
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'fk_items_edition'
    ) THEN
        ALTER TABLE items
            ADD CONSTRAINT fk_items_edition FOREIGN KEY (edition_id) 
            REFERENCES editions(id) ON DELETE SET NULL;
    END IF;
END $$;

-- =============================================================================
-- Migration complete
-- =============================================================================
