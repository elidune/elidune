-- Rename specimens.identification to barcode
ALTER TABLE specimens RENAME COLUMN identification TO barcode;

-- Clean duplicate barcodes before adding unique constraint:
-- For each duplicate barcode, keep one specimen (prefer one that is currently borrowed, else smallest id),
-- delete others that are not currently borrowed, then delete their item if no other specimen uses it.
DO $$
DECLARE
  dup RECORD;
  keeper_id INT;
  spec_rec RECORD;
  other_specimens_count INT;
BEGIN
  FOR dup IN
    SELECT barcode FROM specimens WHERE barcode IS NOT NULL GROUP BY barcode HAVING COUNT(*) > 1
  LOOP
    -- Keep: borrowed first, else smallest id
    SELECT id INTO keeper_id
    FROM specimens s
    WHERE s.barcode = dup.barcode
    ORDER BY EXISTS(SELECT 1 FROM loans l WHERE l.specimen_id = s.id AND l.returned_date IS NULL) DESC, id ASC
    LIMIT 1;

    -- Delete other specimens with this barcode that are not currently borrowed
    FOR spec_rec IN
      SELECT s.id AS spec_id, s.id_item AS spec_item_id
      FROM specimens s
      WHERE s.barcode = dup.barcode AND s.id != keeper_id
        AND NOT EXISTS (SELECT 1 FROM loans l WHERE l.specimen_id = s.id AND l.returned_date IS NULL)
    LOOP
      DELETE FROM specimens WHERE id = spec_rec.spec_id;

      -- If no other specimen references this item, delete the item
      IF spec_rec.spec_item_id IS NOT NULL THEN
        SELECT COUNT(*) INTO other_specimens_count FROM specimens WHERE id_item = spec_rec.spec_item_id;
        IF other_specimens_count = 0 THEN
          DELETE FROM items WHERE id = spec_rec.spec_item_id;
        END IF;
      END IF;
    END LOOP;
  END LOOP;
END $$;

-- Add unique constraint (NULL allowed, duplicate non-NULL barcodes forbidden)
CREATE UNIQUE INDEX idx_specimens_barcode_unique ON specimens (barcode) WHERE barcode IS NOT NULL;
