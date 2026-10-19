-- Incoming deposit batches (copies lent to us by another library).
--
-- `item_withdrawals.destination = 'deposit'` is the opposite direction:
-- our own copies sent out. This migration does not use that word and
-- does not move any existing row. `items.source_id` stays the site
-- where the copy sits with us.
--
-- SQLx applies this file in one transaction. Do not add BEGIN/COMMIT.
-- Lenders and batches are archived or closed, never deleted while items
-- reference them: no ON DELETE CASCADE.

CREATE TABLE lenders (
    id          BIGSERIAL PRIMARY KEY,
    name        TEXT NOT NULL,
    contact     TEXT,
    notes       TEXT,
    archived_at TIMESTAMPTZ,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX lenders_name_active_uniq ON lenders (lower(name)) WHERE archived_at IS NULL;

CREATE TABLE deposit_batches (
    id             BIGSERIAL PRIMARY KEY,
    lender_id      BIGINT NOT NULL REFERENCES lenders(id),
    reference      TEXT NOT NULL,              -- agreement / batch reference from the lender
    site_id        BIGINT NOT NULL REFERENCES sources(id),  -- where the batch sits with us
    arrived_on     DATE NOT NULL,
    return_due_on  DATE NOT NULL,
    status         VARCHAR(16) NOT NULL DEFAULT 'active'
                   CHECK (status IN ('active', 'returning', 'closed')),
    closed_at      TIMESTAMPTZ,
    notes          TEXT,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (return_due_on >= arrived_on),
    CHECK ((status = 'closed') = (closed_at IS NOT NULL)),
    UNIQUE (lender_id, reference)
);

ALTER TABLE items
    ADD COLUMN deposit_batch_id     BIGINT REFERENCES deposit_batches(id),  -- NULL = own stock
    ADD COLUMN deposit_returned_at  TIMESTAMPTZ;                            -- set by scan-out
ALTER TABLE items ADD CONSTRAINT items_deposit_returned_needs_batch
    CHECK (deposit_returned_at IS NULL OR deposit_batch_id IS NOT NULL);
CREATE INDEX idx_items_deposit_batch ON items (deposit_batch_id) WHERE deposit_batch_id IS NOT NULL;
