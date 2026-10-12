-- Lost copy found at the desk (#146).
-- The replacement charge remembers the copy so check-in can find it after the
-- loan row is archived. refunded_amount is what was given back; it never
-- exceeds what the patron actually paid.
-- Elidune does not split a non-refundable processing fee from the replacement
-- price. The whole replacement charge is cancelled or refunded.

ALTER TABLE fines
    ADD COLUMN IF NOT EXISTS item_id BIGINT REFERENCES items (id) ON DELETE SET NULL;

ALTER TABLE fines
    ADD COLUMN IF NOT EXISTS refunded_amount NUMERIC(12, 2) NOT NULL DEFAULT 0
        CHECK (refunded_amount >= 0);

ALTER TABLE fines DROP CONSTRAINT IF EXISTS fines_refunded_amount_lte_paid;
ALTER TABLE fines
    ADD CONSTRAINT fines_refunded_amount_lte_paid
    CHECK (refunded_amount <= paid_amount);

ALTER TABLE fines DROP CONSTRAINT IF EXISTS fines_status_check;
ALTER TABLE fines
    ADD CONSTRAINT fines_status_check
    CHECK (status IN ('pending', 'partial', 'paid', 'waived', 'cancelled', 'refunded'));

CREATE INDEX IF NOT EXISTS idx_fines_item_replacement
    ON fines (item_id)
    WHERE charge_type = 'replacement' AND item_id IS NOT NULL;

COMMENT ON COLUMN fines.item_id IS
    'Copy this charge belongs to. Set on replacement and damage bills so a lost copy can be resolved after the loan row is archived.';

COMMENT ON COLUMN fines.refunded_amount IS
    'Amount refunded when a collected charge is reversed because the copy was found. Never exceeds paid_amount.';

COMMENT ON COLUMN fines.status IS
    'pending, partial, paid, waived, cancelled (uncollected charge voided), or refunded (collected amount returned).';
