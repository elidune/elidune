-- Patron purchase suggestions. Accepted rows open a draft purchase-order line
-- (title/author intent). They do not create biblios or items.
-- Draft purchase orders may exist without a vendor until staff assign one.

ALTER TABLE purchase_orders
    ALTER COLUMN vendor_id DROP NOT NULL;

CREATE TABLE IF NOT EXISTS purchase_suggestions (
    id                      BIGINT      PRIMARY KEY,
    proposed_by             BIGINT      NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    title                   VARCHAR(500) NOT NULL,
    author                  VARCHAR(300) NOT NULL,
    comment                 TEXT,
    status                  VARCHAR(20) NOT NULL DEFAULT 'proposed',
    staff_note              TEXT,
    reviewed_by             BIGINT      REFERENCES users(id) ON DELETE SET NULL,
    reviewed_at             TIMESTAMPTZ,
    purchase_order_id       BIGINT      REFERENCES purchase_orders(id) ON DELETE SET NULL,
    purchase_order_line_id  BIGINT      REFERENCES purchase_order_lines(id) ON DELETE SET NULL,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT purchase_suggestions_status_chk
        CHECK (status IN ('proposed', 'accepted', 'refused')),
    CONSTRAINT purchase_suggestions_title_chk CHECK (length(btrim(title)) > 0),
    CONSTRAINT purchase_suggestions_author_chk CHECK (length(btrim(author)) > 0)
);

CREATE INDEX IF NOT EXISTS idx_purchase_suggestions_status
    ON purchase_suggestions (status);
CREATE INDEX IF NOT EXISTS idx_purchase_suggestions_proposed_by
    ON purchase_suggestions (proposed_by);
CREATE INDEX IF NOT EXISTS idx_purchase_suggestions_created
    ON purchase_suggestions (created_at DESC);

COMMENT ON TABLE purchase_suggestions IS
    'Patron title proposals. Accept opens a draft purchase-order line (intent); refuse stays refused. No biblio or item is created here.';
