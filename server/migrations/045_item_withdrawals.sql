-- Withdrawal events for weeding (#139). Extends #56/#68: items.weeding_status
-- stays the circulable flag; the reason, destination, and staff member live here.
-- At most one active (non-cancelled) withdrawal per item.
-- Safe to re-run: constraints are dropped before add, backfill skips existing active rows.

ALTER TABLE account_types
    ADD COLUMN IF NOT EXISTS weeding_rights VARCHAR(1) NOT NULL DEFAULT 'n',
    ADD COLUMN IF NOT EXISTS weeding_cancel_rights VARCHAR(1) NOT NULL DEFAULT 'n';

UPDATE account_types
SET weeding_rights = 'w',
    weeding_cancel_rights = 'w'
WHERE code IN ('librarian', 'admin');

COMMENT ON COLUMN account_types.weeding_rights IS
    'n/r/w: withdraw copies, record disposal, complete legacy rows, list and export. Distinct from items_rights.';
COMMENT ON COLUMN account_types.weeding_cancel_rights IS
    'n/r/w: cancel an active withdrawal that is not yet disposed. Write is required. Distinct from weeding_rights.';
COMMENT ON COLUMN account_types.items_archive_rights IS
    'n/r/w: delete a physical copy (DELETE /items/{id}). Distinct from weeding_rights.';

CREATE TABLE IF NOT EXISTS item_withdrawals (
    id             BIGINT PRIMARY KEY,
    item_id        BIGINT NOT NULL REFERENCES items (id) ON DELETE RESTRICT,
    withdrawn_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    reason         VARCHAR(32) NOT NULL,
    reason_detail  TEXT,
    destination    VARCHAR(32),
    agent_id       BIGINT REFERENCES users (id) ON DELETE SET NULL,
    cancelled_at   TIMESTAMPTZ,
    cancelled_by   BIGINT REFERENCES users (id) ON DELETE SET NULL,
    disposed_at    TIMESTAMPTZ,
    disposed_by    BIGINT REFERENCES users (id) ON DELETE SET NULL,
    legacy         BOOLEAN NOT NULL DEFAULT FALSE
);

ALTER TABLE item_withdrawals
    ADD COLUMN IF NOT EXISTS disposed_at TIMESTAMPTZ;
ALTER TABLE item_withdrawals
    ADD COLUMN IF NOT EXISTS disposed_by BIGINT REFERENCES users (id) ON DELETE SET NULL;

ALTER TABLE item_withdrawals DROP CONSTRAINT IF EXISTS item_withdrawals_reason_chk;
ALTER TABLE item_withdrawals
    ADD CONSTRAINT item_withdrawals_reason_chk
    CHECK (reason IN (
        'worn', 'obsolete', 'new_edition', 'surplus', 'not_borrowed',
        'out_of_policy', 'lost_not_replaced', 'other'
    ));

ALTER TABLE item_withdrawals DROP CONSTRAINT IF EXISTS item_withdrawals_destination_chk;
ALTER TABLE item_withdrawals
    ADD CONSTRAINT item_withdrawals_destination_chk
    CHECK (destination IS NULL OR destination IN ('pulp', 'donation', 'sale', 'deposit'));

ALTER TABLE item_withdrawals DROP CONSTRAINT IF EXISTS item_withdrawals_other_detail_chk;
ALTER TABLE item_withdrawals
    ADD CONSTRAINT item_withdrawals_other_detail_chk
    CHECK (
        reason <> 'other'
        OR (
            reason_detail IS NOT NULL
            AND char_length(btrim(reason_detail)) >= 3
        )
    );

ALTER TABLE item_withdrawals DROP CONSTRAINT IF EXISTS item_withdrawals_destination_required_chk;
ALTER TABLE item_withdrawals
    ADD CONSTRAINT item_withdrawals_destination_required_chk
    CHECK (legacy OR destination IS NOT NULL);

CREATE UNIQUE INDEX IF NOT EXISTS idx_item_withdrawals_one_active
    ON item_withdrawals (item_id)
    WHERE cancelled_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_item_withdrawals_withdrawn_at
    ON item_withdrawals (withdrawn_at DESC);

CREATE INDEX IF NOT EXISTS idx_item_withdrawals_reason
    ON item_withdrawals (reason);

COMMENT ON TABLE item_withdrawals IS
    'Weeding event for one physical copy. Cancel keeps the row. Legacy rows are the #56/#68 backfill (destination may be null).';
COMMENT ON COLUMN item_withdrawals.legacy IS
    'True when the row was backfilled from items.weeding_status/weeding_reason. Destination may be completed later.';
COMMENT ON COLUMN item_withdrawals.destination IS
    'pulp, donation, sale, or deposit. NULL only on legacy rows that were not yet completed.';
COMMENT ON COLUMN item_withdrawals.disposed_at IS
    'When the copy physically left the library. NULL until staff record disposal. Legacy #56/#68 rows use withdrawn_at.';
COMMENT ON COLUMN item_withdrawals.disposed_by IS
    'Staff member who recorded disposal. NULL on the #56/#68 backfill and until disposal is recorded.';

-- #56/#68 rows: reason other, free text as detail, destination NULL, legacy true.
-- Those copies are already gone, so disposed_at is the withdrawal date and disposed_by stays NULL.
INSERT INTO item_withdrawals (
    id, item_id, withdrawn_at, reason, reason_detail, destination, agent_id, legacy,
    disposed_at, disposed_by
)
SELECT
    (SELECT COALESCE(MAX(id), 0) FROM item_withdrawals)
        + ROW_NUMBER() OVER (ORDER BY i.id),
    i.id,
    COALESCE(i.updated_at, i.created_at, NOW()),
    'other',
    CASE
        WHEN i.weeding_reason IS NOT NULL AND char_length(btrim(i.weeding_reason)) >= 3
            THEN btrim(i.weeding_reason)
        WHEN i.weeding_reason IS NOT NULL AND char_length(btrim(i.weeding_reason)) > 0
            THEN btrim(i.weeding_reason) || ' (reprise)'
        ELSE 'reprise'
    END,
    NULL,
    NULL,
    TRUE,
    COALESCE(i.updated_at, i.created_at, NOW()),
    NULL
FROM items i
WHERE i.weeding_status = 'withdrawn'
  AND NOT EXISTS (
      SELECT 1 FROM item_withdrawals w
      WHERE w.item_id = i.id AND w.cancelled_at IS NULL
  );

UPDATE item_withdrawals
SET disposed_at = withdrawn_at
WHERE legacy AND disposed_at IS NULL;
