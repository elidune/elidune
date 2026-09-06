-- Holds queue at bibliographic (notice) level. A concrete copy is allocated
-- later when a specimen becomes available. Inter-site transit (#15) is an
-- item movement, not a second reservation type.
--
-- See docs/README-holds.md.

ALTER TABLE holds
    ADD COLUMN IF NOT EXISTS biblio_id BIGINT REFERENCES biblios (id) ON DELETE CASCADE,
    ADD COLUMN IF NOT EXISTS pickup_site_id BIGINT;

UPDATE holds h
SET biblio_id = i.biblio_id
FROM items i
WHERE h.item_id = i.id
  AND h.biblio_id IS NULL;

ALTER TABLE holds
    ALTER COLUMN biblio_id SET NOT NULL,
    ALTER COLUMN item_id DROP NOT NULL;

COMMENT ON COLUMN holds.biblio_id IS
    'Bibliographic record (notice) this hold is queued on. Always set.';
COMMENT ON COLUMN holds.item_id IS
    'Allocated copy. NULL while waiting in the title queue; set when staff pin a specimen or when fulfillment assigns one.';
COMMENT ON COLUMN holds.pickup_site_id IS
    'Optional pickup site for later inter-site transit (#15). No sites FK yet.';

-- One active hold per patron per title (the queue unit).
DROP INDEX IF EXISTS idx_holds_one_active_per_user_item;
CREATE UNIQUE INDEX IF NOT EXISTS idx_holds_one_active_per_user_biblio
    ON holds (user_id, biblio_id)
    WHERE status IN ('pending', 'ready');

CREATE INDEX IF NOT EXISTS idx_holds_biblio_status ON holds (biblio_id, status);
CREATE INDEX IF NOT EXISTS idx_holds_biblio_queue
    ON holds (biblio_id, position, created_at)
    WHERE status IN ('pending', 'ready');

COMMENT ON INDEX idx_holds_one_active_per_user_biblio IS
    'At most one pending/ready hold per user and bibliographic record.';
