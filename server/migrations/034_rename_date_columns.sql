-- =============================================================================
-- Migration 034: Rename legacy date columns to *_at convention
-- =============================================================================

DO $$
BEGIN
  -- users
  IF EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_name = 'users' AND column_name = 'crea_date'
  ) THEN
    ALTER TABLE users RENAME COLUMN crea_date TO created_at;
  END IF;

  IF EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_name = 'users' AND column_name = 'modif_date'
  ) THEN
    ALTER TABLE users RENAME COLUMN modif_date TO update_at;
  END IF;

  IF EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_name = 'users' AND column_name = 'issue_date'
  ) THEN
    ALTER TABLE users RENAME COLUMN issue_date TO issue_at;
  END IF;

  IF EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_name = 'users' AND column_name = 'archived_date'
  ) THEN
    ALTER TABLE users RENAME COLUMN archived_date TO archived_at;
  END IF;

  -- loans
  IF EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_name = 'loans' AND column_name = 'renew_date'
  ) THEN
    ALTER TABLE loans RENAME COLUMN renew_date TO renew_at;
  END IF;

  IF EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_name = 'loans' AND column_name = 'issue_date'
  ) THEN
    ALTER TABLE loans RENAME COLUMN issue_date TO issue_at;
  END IF;

  IF EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_name = 'loans' AND column_name = 'returned_date'
  ) THEN
    ALTER TABLE loans RENAME COLUMN returned_date TO returned_at;
  END IF;

  -- loans_archives
  IF EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_name = 'loans_archives' AND column_name = 'issue_date'
  ) THEN
    ALTER TABLE loans_archives RENAME COLUMN issue_date TO issue_at;
  END IF;

  IF EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_name = 'loans_archives' AND column_name = 'returned_date'
  ) THEN
    ALTER TABLE loans_archives RENAME COLUMN returned_date TO returned_at;
  END IF;

  -- sources
  IF EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_name = 'sources' AND column_name = 'archive_date'
  ) THEN
    ALTER TABLE sources RENAME COLUMN archive_date TO archived_at;
  END IF;

  -- schedules
  IF EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_name = 'schedule_periods' AND column_name = 'crea_date'
  ) THEN
    ALTER TABLE schedule_periods RENAME COLUMN crea_date TO created_at;
  END IF;

  IF EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_name = 'schedule_periods' AND column_name = 'modif_date'
  ) THEN
    ALTER TABLE schedule_periods RENAME COLUMN modif_date TO update_at;
  END IF;

  IF EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_name = 'schedule_slots' AND column_name = 'crea_date'
  ) THEN
    ALTER TABLE schedule_slots RENAME COLUMN crea_date TO created_at;
  END IF;

  -- equipment
  IF EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_name = 'equipment' AND column_name = 'crea_date'
  ) THEN
    ALTER TABLE equipment RENAME COLUMN crea_date TO created_at;
  END IF;

  IF EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_name = 'equipment' AND column_name = 'modif_date'
  ) THEN
    ALTER TABLE equipment RENAME COLUMN modif_date TO update_at;
  END IF;

  -- events
  IF EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_name = 'events' AND column_name = 'crea_date'
  ) THEN
    ALTER TABLE events RENAME COLUMN crea_date TO created_at;
  END IF;

  IF EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_name = 'events' AND column_name = 'modif_date'
  ) THEN
    ALTER TABLE events RENAME COLUMN modif_date TO update_at;
  END IF;

  -- visitor counts
  IF EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_name = 'visitor_counts' AND column_name = 'crea_date'
  ) THEN
    ALTER TABLE visitor_counts RENAME COLUMN crea_date TO created_at;
  END IF;

  -- items and authors legacy dates
  IF EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_name = 'items' AND column_name = 'crea_date'
  ) THEN
    ALTER TABLE items RENAME COLUMN crea_date TO created_at;
  END IF;

  IF EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_name = 'items' AND column_name = 'modif_date'
  ) THEN
    ALTER TABLE items RENAME COLUMN modif_date TO update_at;
  END IF;

  IF EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_name = 'items' AND column_name = 'archived_date'
  ) THEN
    ALTER TABLE items RENAME COLUMN archived_date TO archived_at;
  END IF;

  IF EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_name = 'authors' AND column_name = 'crea_date'
  ) THEN
    ALTER TABLE authors RENAME COLUMN crea_date TO created_at;
  END IF;

  IF EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_name = 'authors' AND column_name = 'modif_date'
  ) THEN
    ALTER TABLE authors RENAME COLUMN modif_date TO update_at;
  END IF;
END $$;
