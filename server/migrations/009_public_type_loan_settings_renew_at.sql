-- Optional per public-type + media override for renewal due-date anchor (inherits when NULL).
ALTER TABLE public_type_loan_settings
    ADD COLUMN IF NOT EXISTS renew_at VARCHAR(32) NULL
    CONSTRAINT public_type_loan_settings_renew_at_chk
    CHECK (renew_at IS NULL OR renew_at IN ('now', 'at_due_date'));

COMMENT ON COLUMN public_type_loan_settings.renew_at IS
    'Renewal anchor override: NULL = use loans_settings.renew_at for this media type; otherwise now | at_due_date.';
