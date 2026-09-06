-- Fines domain tables (used by FinesService) and unpaid-balance circulation policy.
-- Checkout/renew (#10) read total_unpaid; accrual/scheduler (#18) write pending fines.

CREATE TABLE IF NOT EXISTS fine_rules (
    id          SERIAL PRIMARY KEY,
    media_type  VARCHAR(50),
    daily_rate  NUMERIC(12, 2) NOT NULL CHECK (daily_rate >= 0),
    max_amount  NUMERIC(12, 2) CHECK (max_amount IS NULL OR max_amount >= 0),
    grace_days  INTEGER NOT NULL DEFAULT 0 CHECK (grace_days >= 0),
    notes       TEXT
);

CREATE UNIQUE INDEX IF NOT EXISTS fine_rules_media_type_key
    ON fine_rules (media_type)
    WHERE media_type IS NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS fine_rules_default_key
    ON fine_rules ((TRUE))
    WHERE media_type IS NULL;

CREATE TABLE IF NOT EXISTS fines (
    id          BIGINT PRIMARY KEY,
    loan_id     BIGINT NOT NULL,
    user_id     BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    amount      NUMERIC(12, 2) NOT NULL CHECK (amount >= 0),
    paid_amount NUMERIC(12, 2) NOT NULL DEFAULT 0 CHECK (paid_amount >= 0),
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    paid_at     TIMESTAMPTZ,
    status      VARCHAR(20) NOT NULL DEFAULT 'pending',
    notes       TEXT
);

CREATE INDEX IF NOT EXISTS idx_fines_user_status ON fines (user_id, status);
CREATE INDEX IF NOT EXISTS idx_fines_loan_id ON fines (loan_id);

-- Single-row global circulation policy (patron unpaid-balance gate).
CREATE TABLE IF NOT EXISTS circulation_settings (
    id                      SMALLINT PRIMARY KEY DEFAULT 1 CHECK (id = 1),
    unpaid_fine_threshold   NUMERIC(12, 2) NOT NULL DEFAULT 0
        CHECK (unpaid_fine_threshold >= 0)
);

INSERT INTO circulation_settings (id, unpaid_fine_threshold)
VALUES (1, 0)
ON CONFLICT (id) DO NOTHING;

-- Optional per-audience override; NULL inherits the global threshold.
ALTER TABLE public_types
    ADD COLUMN IF NOT EXISTS unpaid_fine_threshold NUMERIC(12, 2);

ALTER TABLE public_types
    DROP CONSTRAINT IF EXISTS public_types_unpaid_fine_threshold_check;

ALTER TABLE public_types
    ADD CONSTRAINT public_types_unpaid_fine_threshold_check
    CHECK (unpaid_fine_threshold IS NULL OR unpaid_fine_threshold >= 0);
