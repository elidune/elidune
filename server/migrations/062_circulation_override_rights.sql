-- Dedicated circulation override right (#167).
-- Replaces the settings-write check in checkout_refusal_forceable.
-- Every account type that can force today (settings_rights = w), including
-- custom types, keeps that ability. The account-type code is not consulted.

ALTER TABLE account_types
    ADD COLUMN IF NOT EXISTS circulation_override_rights VARCHAR(1) NOT NULL DEFAULT 'n';

UPDATE account_types
SET circulation_override_rights = 'w'
WHERE settings_rights = 'w';

COMMENT ON COLUMN account_types.circulation_override_rights IS
    'n/r/w. Write may force a loan past a hold for another patron and may force checkout or renewal past subscription_expired. A lost card and an open loan stay non-forceable. A reason is required. Distinct from settings_rights.';
