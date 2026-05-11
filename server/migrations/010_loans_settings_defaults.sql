-- Global default row: media_type IS NULL (one row). Per-media rows keep media_type set.
-- public_type_loan_settings: same pattern per public_type_id.

-- ---- loans_settings --------------------------------------------------------
ALTER TABLE loans_settings DROP CONSTRAINT IF EXISTS loans_settings_media_type_key;

ALTER TABLE loans_settings DROP COLUMN IF EXISTS account_type;

ALTER TABLE loans_settings ADD COLUMN IF NOT EXISTS nb_max_total SMALLINT NULL;

COMMENT ON COLUMN loans_settings.nb_max_total IS
    'Cap on total concurrent loans for a patron (only on media_type IS NULL default row; NULL on per-media rows).';

CREATE UNIQUE INDEX IF NOT EXISTS loans_settings_media_type_unique
    ON loans_settings (media_type) WHERE media_type IS NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS loans_settings_default_row
    ON loans_settings ((1)) WHERE media_type IS NULL;

INSERT INTO loans_settings (media_type, nb_max, nb_renews, duration, renew_at, nb_max_total, notes)
SELECT NULL, 5, 2, 21, 'now', 5, ''
WHERE NOT EXISTS (SELECT 1 FROM loans_settings WHERE media_type IS NULL);

-- ---- public_type_loan_settings --------------------------------------------
ALTER TABLE public_type_loan_settings DROP CONSTRAINT IF EXISTS public_type_loan_settings_public_type_id_media_type_key;

ALTER TABLE public_type_loan_settings ALTER COLUMN media_type DROP NOT NULL;

ALTER TABLE public_type_loan_settings ADD COLUMN IF NOT EXISTS nb_max_total SMALLINT NULL;

COMMENT ON COLUMN public_type_loan_settings.nb_max_total IS
    'Patron total-loans cap for this public type (only when media_type IS NULL; NULL on per-media overrides).';

CREATE UNIQUE INDEX IF NOT EXISTS ptls_public_media_unique
    ON public_type_loan_settings (public_type_id, media_type) WHERE media_type IS NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS ptls_public_default_row
    ON public_type_loan_settings (public_type_id) WHERE media_type IS NULL;

INSERT INTO public_type_loan_settings (public_type_id, media_type, duration, nb_max, nb_renews, renew_at, nb_max_total)
SELECT pt.id, NULL, 21, 5, 2, 'now', 5
FROM public_types pt
WHERE NOT EXISTS (
    SELECT 1 FROM public_type_loan_settings x
    WHERE x.public_type_id = pt.id AND x.media_type IS NULL
);
