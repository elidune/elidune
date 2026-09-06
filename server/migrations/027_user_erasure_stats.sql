-- Patron GDPR erasure (#17): archive stats dimensions + allow identity cut on history.
-- Do not rewrite users.id. Snapshot non-identifying dims, then set history user_id to NULL.

ALTER TABLE loans_archives
    ADD COLUMN IF NOT EXISTS age_band VARCHAR(16),
    ADD COLUMN IF NOT EXISTS loan_year SMALLINT;

CREATE INDEX IF NOT EXISTS idx_loans_archives_age_band ON loans_archives (age_band);
CREATE INDEX IF NOT EXISTS idx_loans_archives_loan_year ON loans_archives (loan_year);

UPDATE loans_archives
SET loan_year = EXTRACT(YEAR FROM date)::smallint
WHERE date IS NOT NULL AND loan_year IS NULL;

-- Fines remain after erasure; they must not keep a joinable user_id.
ALTER TABLE fines
    ALTER COLUMN user_id DROP NOT NULL;

ALTER TABLE fines
    DROP CONSTRAINT IF EXISTS fines_user_id_fkey;

ALTER TABLE fines
    ADD CONSTRAINT fines_user_id_fkey
    FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE SET NULL;

-- Backfill dims + cut identity for patrons already soft-deleted with residual PII.
UPDATE loans_archives la
SET
    borrower_public_type = COALESCE(la.borrower_public_type, u.public_type),
    addr_city = COALESCE(la.addr_city, u.addr_city),
    account_type = COALESCE(la.account_type, u.account_type),
    loan_year = COALESCE(la.loan_year, EXTRACT(YEAR FROM la.date)::smallint),
    age_band = COALESCE(
        la.age_band,
        CASE
            WHEN u.birthdate IS NULL OR la.date IS NULL THEN NULL
            WHEN EXTRACT(YEAR FROM AGE(la.date::date, u.birthdate)) < 18 THEN '0-17'
            WHEN EXTRACT(YEAR FROM AGE(la.date::date, u.birthdate)) < 30 THEN '18-29'
            WHEN EXTRACT(YEAR FROM AGE(la.date::date, u.birthdate)) < 50 THEN '30-49'
            WHEN EXTRACT(YEAR FROM AGE(la.date::date, u.birthdate)) < 65 THEN '50-64'
            ELSE '65+'
        END
    ),
    user_id = NULL
FROM users u
WHERE la.user_id = u.id
  AND u.status = 'deleted';

UPDATE fines
SET user_id = NULL
WHERE user_id IN (SELECT id FROM users WHERE status = 'deleted');

-- Full PII / secret scrub on already-deleted stubs. Kept: id, account_type, public_type,
-- created_at, expiry_at, status, archived_at, token_version (incremented).
UPDATE users SET
    login = NULL,
    password = NULL,
    firstname = NULL,
    lastname = NULL,
    email = NULL,
    addr_street = NULL,
    addr_zip_code = NULL,
    addr_city = NULL,
    phone = NULL,
    fee = NULL,
    group_id = NULL,
    barcode = NULL,
    notes = NULL,
    birthdate = NULL,
    language = NULL,
    sex = NULL,
    staff_type = NULL,
    hours_per_week = NULL,
    staff_start_date = NULL,
    staff_end_date = NULL,
    receive_reminders = FALSE,
    two_factor_enabled = FALSE,
    two_factor_method = NULL,
    totp_secret = NULL,
    recovery_codes = NULL,
    recovery_codes_used = NULL,
    must_change_password = FALSE,
    token_version = token_version + 1,
    archived_at = COALESCE(archived_at, NOW()),
    update_at = NOW()
WHERE status = 'deleted';

COMMENT ON COLUMN loans_archives.age_band IS
    'Non-identifying age band at loan date (0-17, 18-29, 30-49, 50-64, 65+). Snapshotted for stats after patron erasure.';
COMMENT ON COLUMN loans_archives.loan_year IS
    'Calendar year of the loan date; stats dimension retained after user_id is nulled.';
