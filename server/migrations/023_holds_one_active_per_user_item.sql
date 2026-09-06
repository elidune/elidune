-- At most one active (pending/ready) hold per patron + physical copy.
-- Concurrent place_hold used to check-then-insert without a unique constraint.
-- Extra active rows are cancelled (oldest created_at / id kept).

WITH dupes AS (
    SELECT id
    FROM (
        SELECT id,
               ROW_NUMBER() OVER (
                   PARTITION BY user_id, item_id
                   ORDER BY created_at ASC NULLS LAST, id ASC
               ) AS rn
        FROM holds
        WHERE status IN ('pending', 'ready')
    ) ranked
    WHERE rn > 1
)
UPDATE holds h
SET status = 'cancelled'
FROM dupes d
WHERE h.id = d.id;

CREATE UNIQUE INDEX IF NOT EXISTS idx_holds_one_active_per_user_item
    ON holds (user_id, item_id)
    WHERE status IN ('pending', 'ready');

COMMENT ON INDEX idx_holds_one_active_per_user_item IS
    'At most one pending/ready hold per user and item (physical copy).';
