-- Per-copy notice that the lender was told a deposit copy was lost or damaged.
-- Replaces lenders.loss_flagged_at (migration 066).
--
-- No backfill. That flag was not used in production. Any value stored there is dropped.

ALTER TABLE items
    ADD COLUMN deposit_loss_notified_at TIMESTAMPTZ;

ALTER TABLE items
    ADD CONSTRAINT items_deposit_loss_notified_needs_batch
    CHECK (deposit_loss_notified_at IS NULL OR deposit_batch_id IS NOT NULL);

ALTER TABLE lenders
    DROP COLUMN loss_flagged_at;
