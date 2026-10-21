-- Permanent deposit batches may omit return_due_on until staff set a date.

ALTER TABLE deposit_batches
    ALTER COLUMN return_due_on DROP NOT NULL;

ALTER TABLE deposit_batches
    DROP CONSTRAINT IF EXISTS deposit_batches_check,
    DROP CONSTRAINT IF EXISTS deposit_batches_check1;

ALTER TABLE deposit_batches
    ADD CONSTRAINT deposit_batches_return_due_on_check
        CHECK (return_due_on IS NULL OR return_due_on >= arrived_on),
    ADD CONSTRAINT deposit_batches_closed_at_check
        CHECK ((status = 'closed') = (closed_at IS NOT NULL));
