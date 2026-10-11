-- Shelf pick status for title holds.
--
-- A title hold placed while a copy is on the shelf is assigned that copy and
-- stored as `to_pick` (à prélever). Staff pull it from the pick list; the hold
-- becomes `ready` (same site) or enters the existing transit flow only when
-- that copy is checked in. A returned copy is already in staff hands and still
-- goes straight to `ready` / transit — it is not `to_pick`.
--
-- `to_pick` is an active hold: it counts toward the cap and the one-active-hold
-- uniqueness rule, and it reserves only the assigned copy.

ALTER TABLE holds DROP CONSTRAINT IF EXISTS holds_status_chk;

ALTER TABLE holds
    ADD CONSTRAINT holds_status_chk
    CHECK (status IN (
        'pending',
        'to_pick',
        'ready',
        'fulfilled',
        'cancelled',
        'expired'
    ));

COMMENT ON COLUMN holds.status IS
    'pending: waiting for a copy. to_pick: copy assigned, still on the shelf (no patron notice). ready: patron notified. fulfilled / cancelled / expired: closed.';

DROP INDEX IF EXISTS idx_holds_one_active_per_user_biblio;
CREATE UNIQUE INDEX idx_holds_one_active_per_user_biblio
    ON holds (user_id, biblio_id)
    WHERE status IN ('pending', 'to_pick', 'ready');

DROP INDEX IF EXISTS idx_holds_biblio_queue;
CREATE INDEX idx_holds_biblio_queue
    ON holds (biblio_id, position, created_at)
    WHERE status IN ('pending', 'to_pick', 'ready');

COMMENT ON INDEX idx_holds_one_active_per_user_biblio IS
    'At most one pending/to_pick/ready hold per user and bibliographic record.';
