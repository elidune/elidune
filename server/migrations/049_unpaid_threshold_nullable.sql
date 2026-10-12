-- Amount-based unpaid-fine blocking is off unless a threshold is stored.
--
-- Effective threshold:
--   public_types.unpaid_fine_threshold when non-null,
--   else circulation_settings.unpaid_fine_threshold when non-null,
--   else no amount-based block.
-- A stored 0 still blocks any strictly positive unpaid balance.
-- Overdue loans and a manually blocked account are unchanged.
--
-- circulation_settings has no created_at or updated_at. The application writes
-- this column only from PUT /fines/policy, which inserts audit_log
-- event_type = 'fine.policy_updated', entity_type = 'circulation_settings',
-- entity_id = the settings id, outcome = 'success' when the write commits.
-- Convert a stored 0 to NULL only when no such successful audit row exists
-- (the migration 025 seed). A failure audit does not count: the value did not change.
--
-- public_types has no created_at or updated_at. unpaid_fine_threshold was added
-- nullable in migration 025 and was never defaulted to 0, so a stored 0 is an
-- explicit override. This migration does not update public_types rows.

ALTER TABLE circulation_settings
    ALTER COLUMN unpaid_fine_threshold DROP NOT NULL,
    ALTER COLUMN unpaid_fine_threshold SET DEFAULT NULL;

ALTER TABLE circulation_settings
    DROP CONSTRAINT IF EXISTS circulation_settings_unpaid_fine_threshold_check;

ALTER TABLE circulation_settings
    ADD CONSTRAINT circulation_settings_unpaid_fine_threshold_check
    CHECK (unpaid_fine_threshold IS NULL OR unpaid_fine_threshold >= 0);

COMMENT ON COLUMN circulation_settings.unpaid_fine_threshold IS
    'Unpaid balance strictly above this amount blocks checkout and renew. NULL disables amount-based blocking. 0 blocks any positive unpaid balance. public_types.unpaid_fine_threshold overrides when set.';

-- BEGIN UNTOUCHED DEFAULT THRESHOLD
UPDATE circulation_settings AS settings
SET unpaid_fine_threshold = NULL
WHERE settings.unpaid_fine_threshold = 0
  AND NOT EXISTS (
      SELECT 1
      FROM audit_log
      WHERE event_type = 'fine.policy_updated'
        AND entity_type = 'circulation_settings'
        AND entity_id = settings.id
        AND outcome = 'success'
  );
-- END UNTOUCHED DEFAULT THRESHOLD

ALTER TABLE public_types
    ALTER COLUMN unpaid_fine_threshold SET DEFAULT NULL;

COMMENT ON COLUMN public_types.unpaid_fine_threshold IS
    'Optional override. NULL inherits circulation_settings.unpaid_fine_threshold; NULL there means no amount-based block. 0 blocks any positive unpaid balance for this audience. Existing rows are not rewritten.';
