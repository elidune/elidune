-- Shelf-level deposit replacement/damage charges have no patron loan.
-- Keep loan_id when the charge was created on an open loan; NULL otherwise.

ALTER TABLE fines
    ALTER COLUMN loan_id DROP NOT NULL;

COMMENT ON COLUMN fines.loan_id IS
    'Owning loan when the charge was created on an open loan. NULL for shelf-level deposit replacement or damage charges with no patron loan.';

DROP INDEX IF EXISTS idx_fines_one_open_per_loan_charge;

CREATE UNIQUE INDEX idx_fines_one_open_per_loan_charge
    ON fines USING btree (loan_id, charge_type)
    WHERE loan_id IS NOT NULL
      AND ((status)::text = ANY (ARRAY['pending'::character varying, 'partial'::character varying]::text[]));

CREATE UNIQUE INDEX idx_fines_one_open_per_item_charge_no_loan
    ON fines USING btree (item_id, charge_type)
    WHERE loan_id IS NULL
      AND item_id IS NOT NULL
      AND ((status)::text = ANY (ARRAY['pending'::character varying, 'partial'::character varying]::text[]));
