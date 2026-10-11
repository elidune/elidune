-- Renewal extends the current due date, and is only allowed inside a window
-- before that date (default 7 days). Overdue loans stay renewable.
--
-- `renew_at = 'now'` was the previous column default. Rows still holding that
-- value move to `at_due_date`. An explicit future choice of `now` can be saved
-- again through the loans settings API.

ALTER TABLE loans_settings
    ADD COLUMN IF NOT EXISTS renew_window_days SMALLINT NOT NULL DEFAULT 7;

ALTER TABLE loans_settings DROP CONSTRAINT IF EXISTS loans_settings_renew_window_days_chk;
ALTER TABLE loans_settings
    ADD CONSTRAINT loans_settings_renew_window_days_chk
    CHECK (renew_window_days >= 0 AND renew_window_days <= 366);

COMMENT ON COLUMN loans_settings.renew_window_days IS
    'Days before the due date when renewal becomes allowed. 0 = on the due date or later. Overdue loans stay renewable.';

UPDATE loans_settings
SET renew_at = 'at_due_date'
WHERE renew_at = 'now';

UPDATE public_type_loan_settings
SET renew_at = 'at_due_date'
WHERE renew_at = 'now';

ALTER TABLE loans_settings ALTER COLUMN renew_at SET DEFAULT 'at_due_date';

COMMENT ON COLUMN loans_settings.renew_at IS
    'Renewal due-date anchor: at_due_date (default) = current expiry + duration; now = renewal time + duration.';
