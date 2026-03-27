-- Rename reservations → holds (terminology). Safe if already migrated.

DO $$
BEGIN
  IF EXISTS (
    SELECT 1 FROM information_schema.tables
    WHERE table_schema = 'public' AND table_name = 'reservations'
  ) AND NOT EXISTS (
    SELECT 1 FROM information_schema.tables
    WHERE table_schema = 'public' AND table_name = 'holds'
  ) THEN
    ALTER TABLE reservations RENAME TO holds;
    ALTER INDEX IF EXISTS idx_reservations_user_id RENAME TO idx_holds_user_id;
    ALTER INDEX IF EXISTS idx_reservations_item_id RENAME TO idx_holds_item_id;
    ALTER INDEX IF EXISTS idx_reservations_item_status RENAME TO idx_holds_item_status;
  END IF;
END $$;

-- Admin dynamic config key in DB (if present)
UPDATE settings SET key = 'holds' WHERE key = 'reservations';
