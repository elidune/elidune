-- Item weeding lifecycle (#56, #63): orthogonal to circulation_status and archived_at.
-- on_shelf (default) and candidate remain circulable; withdrawn is not.
-- Do not reuse archived_at as the withdrawn marker.

ALTER TABLE items
    ADD COLUMN IF NOT EXISTS weeding_status VARCHAR(20) NOT NULL DEFAULT 'on_shelf',
    ADD COLUMN IF NOT EXISTS weeding_reason VARCHAR(200);

ALTER TABLE items DROP CONSTRAINT IF EXISTS items_weeding_status_chk;
ALTER TABLE items
    ADD CONSTRAINT items_weeding_status_chk
    CHECK (weeding_status IN ('on_shelf', 'candidate', 'withdrawn'));

CREATE INDEX IF NOT EXISTS idx_items_weeding_withdrawn
    ON items (biblio_id)
    WHERE weeding_status = 'withdrawn' AND archived_at IS NULL;

COMMENT ON COLUMN items.weeding_status IS
    'Weeding lifecycle, orthogonal to circulation_status and archived_at: on_shelf (default), candidate (still circulable), withdrawn (not circulable).';
COMMENT ON COLUMN items.weeding_reason IS
    'Optional short staff reason for the current weeding status.';
