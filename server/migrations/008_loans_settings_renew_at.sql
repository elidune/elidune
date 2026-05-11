-- When renewing, new due date is computed from either the renewal instant (`now`)
-- or the current due date (`at_due_date`). See `loans_renew`.
ALTER TABLE loans_settings
    ADD COLUMN IF NOT EXISTS renew_at VARCHAR(32) NOT NULL DEFAULT 'now'
    CONSTRAINT loans_settings_renew_at_chk CHECK (renew_at IN ('now', 'at_due_date'));

COMMENT ON COLUMN loans_settings.renew_at IS
    'Renewal due-date anchor: now = renewal time + duration; at_due_date = current expiry + duration.';
