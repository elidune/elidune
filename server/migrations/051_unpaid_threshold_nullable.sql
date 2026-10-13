-- Unpaid-fine blocking is a mode, not a nullable amount.
--
-- circulation_settings.unpaid_threshold_mode: 'none' | 'amount'
-- public_types.unpaid_threshold_mode: 'inherit' | 'none' | 'amount'
--
-- unpaid_fine_threshold is the amount in currency units. It is used only when
-- the mode is 'amount'. Blocking is strict: unpaid balance must be above it.
-- 0 blocks any positive unpaid balance.
--
-- Effective rule:
--   public type 'amount' -> that type's amount
--   public type 'none'   -> no amount-based block
--   public type 'inherit' (or no public type) -> global mode
--     global 'none'   -> no amount-based block
--     global 'amount' -> global amount
--
-- circulation_settings has no created_at or updated_at. The only application
-- write of the global threshold is PUT /fines/policy, audited as
-- event_type = 'fine.policy_updated', entity_type = 'circulation_settings',
-- entity_id = the settings id, outcome = 'success'.
-- An untouched seeded 0 (no such successful audit) becomes mode 'none'.
-- Every other stored value, including an explicit 0 that was saved, becomes
-- mode 'amount' and keeps that amount. A failure audit does not count.
--
-- public_types has no created_at or updated_at. unpaid_fine_threshold was added
-- nullable and was never defaulted to 0. NULL becomes 'inherit'. A stored
-- value, including 0, becomes 'amount'.
--
-- New installs: global mode defaults to 'none'.
-- New public types: mode defaults to 'inherit'.

ALTER TABLE circulation_settings
    ADD COLUMN IF NOT EXISTS unpaid_threshold_mode TEXT;

ALTER TABLE public_types
    ADD COLUMN IF NOT EXISTS unpaid_threshold_mode TEXT;

-- BEGIN UNTOUCHED DEFAULT THRESHOLD
UPDATE circulation_settings AS settings
SET unpaid_threshold_mode = CASE
    WHEN settings.unpaid_fine_threshold = 0
     AND NOT EXISTS (
         SELECT 1
         FROM audit_log
         WHERE event_type = 'fine.policy_updated'
           AND entity_type = 'circulation_settings'
           AND entity_id = settings.id
           AND outcome = 'success'
     )
    THEN 'none'
    ELSE 'amount'
END
WHERE settings.unpaid_threshold_mode IS NULL;

UPDATE public_types
SET unpaid_threshold_mode = CASE
    WHEN unpaid_fine_threshold IS NULL THEN 'inherit'
    ELSE 'amount'
END
WHERE unpaid_threshold_mode IS NULL;
-- END UNTOUCHED DEFAULT THRESHOLD

UPDATE circulation_settings
SET unpaid_threshold_mode = 'none'
WHERE unpaid_threshold_mode IS NULL;

ALTER TABLE circulation_settings
    ALTER COLUMN unpaid_threshold_mode SET DEFAULT 'none',
    ALTER COLUMN unpaid_threshold_mode SET NOT NULL;

ALTER TABLE circulation_settings
    DROP CONSTRAINT IF EXISTS circulation_settings_unpaid_threshold_mode_check;

ALTER TABLE circulation_settings
    ADD CONSTRAINT circulation_settings_unpaid_threshold_mode_check
    CHECK (unpaid_threshold_mode IN ('none', 'amount'));

UPDATE public_types
SET unpaid_threshold_mode = 'inherit'
WHERE unpaid_threshold_mode IS NULL;

ALTER TABLE public_types
    ALTER COLUMN unpaid_threshold_mode SET DEFAULT 'inherit',
    ALTER COLUMN unpaid_threshold_mode SET NOT NULL;

ALTER TABLE public_types
    DROP CONSTRAINT IF EXISTS public_types_unpaid_threshold_mode_check;

ALTER TABLE public_types
    ADD CONSTRAINT public_types_unpaid_threshold_mode_check
    CHECK (unpaid_threshold_mode IN ('inherit', 'none', 'amount'));

COMMENT ON COLUMN circulation_settings.unpaid_threshold_mode IS
    'none: no amount-based block. amount: block when the unpaid balance is strictly above unpaid_fine_threshold. 0 blocks any positive unpaid balance.';

COMMENT ON COLUMN circulation_settings.unpaid_fine_threshold IS
    'Amount used only when unpaid_threshold_mode is amount. 0 blocks any positive unpaid balance.';

COMMENT ON COLUMN public_types.unpaid_threshold_mode IS
    'inherit: use the global circulation mode. none: never block on amount, even when the global mode is amount. amount: block when the unpaid balance is strictly above unpaid_fine_threshold.';

COMMENT ON COLUMN public_types.unpaid_fine_threshold IS
    'Amount used only when unpaid_threshold_mode is amount. Ignored for inherit and none.';
