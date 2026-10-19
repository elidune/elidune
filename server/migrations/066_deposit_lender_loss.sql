-- Lender replacement rate and a staff flag when a deposit copy is lost or damaged.
-- Rate is integer cents. NULL means staff must enter an amount at billing time.

ALTER TABLE lenders
    ADD COLUMN replacement_rate_cents BIGINT,
    ADD COLUMN loss_flagged_at TIMESTAMPTZ;

ALTER TABLE lenders
    ADD CONSTRAINT lenders_replacement_rate_positive
    CHECK (replacement_rate_cents IS NULL OR replacement_rate_cents > 0);
