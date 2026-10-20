-- Barcode uniqueness among active copies only; lender barcode for deposit rotation.
-- SQLx applies this file in one transaction. Do not add BEGIN/COMMIT.

ALTER TABLE items
    ADD COLUMN IF NOT EXISTS lender_barcode VARCHAR(100);

UPDATE items
SET lender_barcode = barcode
WHERE deposit_batch_id IS NOT NULL
  AND lender_barcode IS NULL
  AND barcode IS NOT NULL;

DROP INDEX IF EXISTS idx_items_barcode_unique;

CREATE UNIQUE INDEX idx_items_barcode_active_unique
    ON items (barcode)
    WHERE barcode IS NOT NULL AND archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_items_deposit_lender_barcode
    ON items (deposit_batch_id, lender_barcode)
    WHERE deposit_batch_id IS NOT NULL AND lender_barcode IS NOT NULL;

CREATE TABLE IF NOT EXISTS item_barcode_restore_log (
    id              BIGSERIAL PRIMARY KEY,
    item_id         BIGINT NOT NULL REFERENCES items(id),
    old_barcode     VARCHAR(100),
    restored_barcode VARCHAR(100),
    duplicate_active BOOLEAN NOT NULL DEFAULT FALSE,
    duplicate_archived BOOLEAN NOT NULL DEFAULT FALSE,
    anomaly         BOOLEAN NOT NULL DEFAULT FALSE,
    note            TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX IF NOT EXISTS item_barcode_restore_log_item_uniq
    ON item_barcode_restore_log (item_id);

-- Restore ARCH_<id>_<original> on archived rows (idempotent).
INSERT INTO item_barcode_restore_log (
    item_id, old_barcode, restored_barcode, duplicate_active, duplicate_archived, anomaly, note
)
SELECT
    i.id,
    i.barcode,
    CASE
        WHEN substring(i.barcode FROM '^ARCH_' || i.id::text || '_(.*)$') = '' THEN NULL
        ELSE substring(i.barcode FROM '^ARCH_' || i.id::text || '_(.*)$')
    END,
    EXISTS (
        SELECT 1 FROM items active
        WHERE active.archived_at IS NULL
          AND active.barcode IS NOT DISTINCT FROM (
              NULLIF(substring(i.barcode FROM '^ARCH_' || i.id::text || '_(.*)$'), '')
          )
          AND active.id <> i.id
    ),
    EXISTS (
        SELECT 1 FROM items other
        WHERE other.archived_at IS NOT NULL
          AND other.id <> i.id
          AND other.barcode IS NOT DISTINCT FROM (
              NULLIF(substring(i.barcode FROM '^ARCH_' || i.id::text || '_(.*)$'), '')
          )
    ),
    FALSE,
    'restored from ARCH_ prefix'
FROM items i
WHERE i.archived_at IS NOT NULL
  AND i.barcode ~ ('^ARCH_' || i.id::text || '_.*$')
ON CONFLICT (item_id) DO NOTHING;

UPDATE items i
SET barcode = CASE
        WHEN substring(i.barcode FROM '^ARCH_' || i.id::text || '_(.*)$') = '' THEN NULL
        ELSE substring(i.barcode FROM '^ARCH_' || i.id::text || '_(.*)$')
    END,
    updated_at = now()
WHERE i.archived_at IS NOT NULL
  AND i.barcode ~ ('^ARCH_' || i.id::text || '_.*$')
  AND EXISTS (SELECT 1 FROM item_barcode_restore_log l WHERE l.item_id = i.id AND NOT l.anomaly);

INSERT INTO item_barcode_restore_log (
    item_id, old_barcode, restored_barcode, duplicate_active, duplicate_archived, anomaly, note
)
SELECT
    i.id,
    i.barcode,
    NULL,
    FALSE,
    FALSE,
    TRUE,
    'ARCH_ prefix does not match this item id'
FROM items i
WHERE i.archived_at IS NOT NULL
  AND i.barcode LIKE 'ARCH\_%' ESCAPE '\'
  AND i.barcode !~ ('^ARCH_' || i.id::text || '_.*$')
ON CONFLICT (item_id) DO NOTHING;
