-- One open (pending/partial) fine per loan — idempotent accrual (#18).
-- Tables come from 025_fines_unpaid_threshold.sql; this only adds the unique
-- index and a default rule when none exists.

CREATE UNIQUE INDEX IF NOT EXISTS idx_fines_one_open_per_loan
    ON fines (loan_id)
    WHERE status IN ('pending', 'partial');

ALTER TABLE fines DROP CONSTRAINT IF EXISTS fines_status_check;
ALTER TABLE fines
    ADD CONSTRAINT fines_status_check
    CHECK (status IN ('pending', 'partial', 'paid', 'waived'));

INSERT INTO fine_rules (media_type, daily_rate, max_amount, grace_days, notes)
SELECT NULL, 0.20, 15.00, 3, 'Default overdue fine rule'
WHERE NOT EXISTS (SELECT 1 FROM fine_rules WHERE media_type IS NULL);

COMMENT ON INDEX idx_fines_one_open_per_loan IS
    'Idempotent accrual: one open (pending/partial) fine per loan.';
