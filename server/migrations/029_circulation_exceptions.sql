-- Item circulation exceptions (#14): dedicated fine charge types so a
-- replacement / damage bill can coexist with an open overdue fine.
-- Item status remains items.circulation_status (SMALLINT):
--   NULL/0 = available, 1 = lost, 2 = damaged, 3 = claimed-returned.

ALTER TABLE fines
    ADD COLUMN IF NOT EXISTS charge_type VARCHAR(20) NOT NULL DEFAULT 'overdue';

ALTER TABLE fines DROP CONSTRAINT IF EXISTS fines_charge_type_check;
ALTER TABLE fines
    ADD CONSTRAINT fines_charge_type_check
    CHECK (charge_type IN ('overdue', 'replacement', 'damage'));

DROP INDEX IF EXISTS idx_fines_one_open_per_loan;

CREATE UNIQUE INDEX IF NOT EXISTS idx_fines_one_open_per_loan_charge
    ON fines (loan_id, charge_type)
    WHERE status IN ('pending', 'partial');

COMMENT ON COLUMN fines.charge_type IS
    'overdue (accrual), replacement (lost), or damage (repair/fee). One open fine per (loan, type).';

COMMENT ON COLUMN items.circulation_status IS
    'Item-level state machine: NULL/0 available, 1 lost, 2 damaged, 3 claimed-returned. Independent of the loan row.';
