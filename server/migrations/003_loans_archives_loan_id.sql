-- fines.loan_id stores the original loans.id. Archiving deletes that row and
-- inserts loans_archives with its own sequence, so loans_archives.id is not
-- the loan id. Keep the original id so an unpaid fine can still be tied to
-- the copy. Rows archived before this column existed stay NULL.

ALTER TABLE loans_archives
    ADD COLUMN loan_id bigint;

COMMENT ON COLUMN loans_archives.loan_id IS
    'Original loans.id, copied when the loan is archived. Not a foreign key: the loans row is deleted. NULL on rows archived before this column existed.';

CREATE INDEX idx_loans_archives_loan_id ON loans_archives (loan_id);
