-- Exclusive copy assignment, pick delay, and the shelf "to search" state.
--
-- At most one hold may exclusively assign a copy while it is `to_pick` or
-- `ready`. There is no `holds.status` value `in_transit`: a copy moving between
-- sites stays a `pending` hold, and exclusivity of that movement is
-- `item_transits.status` (`requested` / `in_transit`) via
-- `idx_item_transits_one_active_per_item`. Assignment still locks the item
-- (`FOR UPDATE` / `SKIP LOCKED`); this index is the same last line of defense
-- as `idx_loans_one_active_per_item`. A violation is retried onto the next
-- copy, not returned as HTTP 500.
--
-- `items.circulation_status = 4` (`to_search`) is a copy missing from the shelf
-- during a pick. It is not lendable and not assignable. A later scan clears it
-- back to available and runs the normal return/queue path. Declaring the copy
-- lost uses the existing lost transition.

CREATE UNIQUE INDEX IF NOT EXISTS idx_holds_one_exclusive_assignment_per_item
    ON holds (item_id)
    WHERE status IN ('to_pick', 'ready');

COMMENT ON INDEX idx_holds_one_exclusive_assignment_per_item IS
    'At most one to_pick or ready hold per copy. In-transit exclusivity lives on item_transits, not holds.status.';

ALTER TABLE holds
    ADD COLUMN IF NOT EXISTS pick_started_at TIMESTAMPTZ;

UPDATE holds
SET pick_started_at = created_at
WHERE status = 'to_pick'
  AND pick_started_at IS NULL;

COMMENT ON COLUMN holds.pick_started_at IS
    'When the current to_pick assignment started. Cleared when the hold leaves to_pick. Due date is this plus circulation_settings.pick_delay_days. No automatic expiry.';

ALTER TABLE circulation_settings
    ADD COLUMN IF NOT EXISTS pick_delay_days SMALLINT NOT NULL DEFAULT 2;

ALTER TABLE circulation_settings
    DROP CONSTRAINT IF EXISTS circulation_settings_pick_delay_days_check;

ALTER TABLE circulation_settings
    ADD CONSTRAINT circulation_settings_pick_delay_days_check
    CHECK (pick_delay_days >= 0 AND pick_delay_days <= 365);

COMMENT ON COLUMN circulation_settings.pick_delay_days IS
    'Days after pick_started_at before a to_pick row is flagged overdue on the pick list. Default 2. Does not expire or cancel the hold.';
