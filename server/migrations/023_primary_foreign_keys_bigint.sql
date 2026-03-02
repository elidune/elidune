-- =============================================================================
-- Migration 023: Convert all primary and foreign key columns to BIGINT
-- =============================================================================
-- Purpose: Align PK/FK with u64 in Rust (BIGINT in PostgreSQL).
-- Phase 1: All id (PK) columns. Phase 2: All *_id (FK) columns.
-- =============================================================================

-- =============================================================================
-- Phase 1: Primary key columns (id) -> BIGINT
-- =============================================================================

ALTER TABLE authors ALTER COLUMN id TYPE BIGINT USING id::bigint;
ALTER TABLE items ALTER COLUMN id TYPE BIGINT USING id::bigint;
ALTER TABLE specimens ALTER COLUMN id TYPE BIGINT USING id::bigint;
ALTER TABLE sources ALTER COLUMN id TYPE BIGINT USING id::bigint;
ALTER TABLE series ALTER COLUMN id TYPE BIGINT USING id::bigint;
ALTER TABLE collections ALTER COLUMN id TYPE BIGINT USING id::bigint;
ALTER TABLE editions ALTER COLUMN id TYPE BIGINT USING id::bigint;
ALTER TABLE item_authors ALTER COLUMN id TYPE BIGINT USING id::bigint;
ALTER TABLE users ALTER COLUMN id TYPE BIGINT USING id::bigint;
ALTER TABLE loans ALTER COLUMN id TYPE BIGINT USING id::bigint;
ALTER TABLE visitor_counts ALTER COLUMN id TYPE BIGINT USING id::bigint;
ALTER TABLE schedule_periods ALTER COLUMN id TYPE BIGINT USING id::bigint;
ALTER TABLE schedule_slots ALTER COLUMN id TYPE BIGINT USING id::bigint;
ALTER TABLE schedule_closures ALTER COLUMN id TYPE BIGINT USING id::bigint;
ALTER TABLE equipment ALTER COLUMN id TYPE BIGINT USING id::bigint;
ALTER TABLE events ALTER COLUMN id TYPE BIGINT USING id::bigint;

-- loans_archives and z3950servers may exist
DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = 'loans_archives') THEN
    ALTER TABLE loans_archives ALTER COLUMN id TYPE BIGINT USING id::bigint;
  END IF;
END $$;
DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = 'z3950servers') THEN
    ALTER TABLE z3950servers ALTER COLUMN id TYPE BIGINT USING id::bigint;
  END IF;
END $$;

-- =============================================================================
-- Phase 2: Foreign key columns (*_id) -> BIGINT
-- =============================================================================

-- items
ALTER TABLE items ALTER COLUMN series_id TYPE BIGINT USING series_id::bigint;
ALTER TABLE items ALTER COLUMN edition_id TYPE BIGINT USING edition_id::bigint;
ALTER TABLE items ALTER COLUMN collection_id TYPE BIGINT USING collection_id::bigint;

-- specimens
ALTER TABLE specimens ALTER COLUMN item_id TYPE BIGINT USING item_id::bigint;
ALTER TABLE specimens ALTER COLUMN source_id TYPE BIGINT USING source_id::bigint;

-- item_authors
ALTER TABLE item_authors ALTER COLUMN item_id TYPE BIGINT USING item_id::bigint;
ALTER TABLE item_authors ALTER COLUMN author_id TYPE BIGINT USING author_id::bigint;

-- users
ALTER TABLE users ALTER COLUMN group_id TYPE BIGINT USING group_id::bigint;

-- loans
ALTER TABLE loans ALTER COLUMN user_id TYPE BIGINT USING user_id::bigint;
ALTER TABLE loans ALTER COLUMN specimen_id TYPE BIGINT USING specimen_id::bigint;

-- schedule_slots
ALTER TABLE schedule_slots ALTER COLUMN period_id TYPE BIGINT USING period_id::bigint;

-- loans_archives (if exists)
DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = 'loans_archives') THEN
    ALTER TABLE loans_archives ALTER COLUMN user_id TYPE BIGINT USING user_id::bigint;
    ALTER TABLE loans_archives ALTER COLUMN specimen_id TYPE BIGINT USING specimen_id::bigint;
  END IF;
END $$;
