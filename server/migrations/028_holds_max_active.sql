-- Max concurrent active holds (pending/ready) per patron.
-- Global default on circulation_settings; optional public-types override (NULL inherits).

ALTER TABLE circulation_settings
    ADD COLUMN IF NOT EXISTS max_active_holds SMALLINT NOT NULL DEFAULT 20;

ALTER TABLE circulation_settings
    DROP CONSTRAINT IF EXISTS circulation_settings_max_active_holds_check;

ALTER TABLE circulation_settings
    ADD CONSTRAINT circulation_settings_max_active_holds_check
    CHECK (max_active_holds >= 1);

COMMENT ON COLUMN circulation_settings.max_active_holds IS
    'Default maximum pending+ready holds per patron. public_types.max_active_holds overrides when set.';

ALTER TABLE public_types
    ADD COLUMN IF NOT EXISTS max_active_holds SMALLINT;

ALTER TABLE public_types
    DROP CONSTRAINT IF EXISTS public_types_max_active_holds_check;

ALTER TABLE public_types
    ADD CONSTRAINT public_types_max_active_holds_check
    CHECK (max_active_holds IS NULL OR max_active_holds >= 1);

COMMENT ON COLUMN public_types.max_active_holds IS
    'Optional max pending+ready holds for this audience. NULL inherits circulation_settings.max_active_holds.';
