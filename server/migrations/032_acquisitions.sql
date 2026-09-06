-- Acquisitions domain (vendors, yearly funds, purchase orders, receipts).
-- Separate from cataloging; physical items are created only at receipt.

ALTER TABLE account_types
    ADD COLUMN IF NOT EXISTS acquisitions_rights VARCHAR(1) DEFAULT 'n';

UPDATE account_types
SET acquisitions_rights = 'w'
WHERE code IN ('librarian', 'admin');

UPDATE account_types
SET acquisitions_rights = 'n'
WHERE acquisitions_rights IS NULL;

COMMENT ON COLUMN account_types.acquisitions_rights IS
    'n/r/w: none, read, or write access to /acquisitions (distinct from items_rights / cataloging)';

CREATE TABLE IF NOT EXISTS vendors (
    id          BIGINT      PRIMARY KEY,
    name        VARCHAR(200) NOT NULL,
    code        VARCHAR(50),
    email       VARCHAR(200),
    phone       VARCHAR(50),
    address     TEXT,
    notes       TEXT,
    active      BOOLEAN     NOT NULL DEFAULT TRUE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    archived_at TIMESTAMPTZ
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_vendors_code_active
    ON vendors (lower(code))
    WHERE code IS NOT NULL AND archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_vendors_name ON vendors (lower(name));

CREATE TABLE IF NOT EXISTS acquisition_funds (
    id                BIGINT         PRIMARY KEY,
    code              VARCHAR(50)    NOT NULL,
    name              VARCHAR(200)   NOT NULL,
    fiscal_year       INTEGER        NOT NULL,
    allocated_amount  NUMERIC(14, 2) NOT NULL DEFAULT 0,
    currency          VARCHAR(3)     NOT NULL DEFAULT 'EUR',
    notes             TEXT,
    created_at        TIMESTAMPTZ    NOT NULL DEFAULT NOW(),
    updated_at        TIMESTAMPTZ    NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_acquisition_funds_code_year
    ON acquisition_funds (lower(code), fiscal_year);
CREATE INDEX IF NOT EXISTS idx_acquisition_funds_year ON acquisition_funds (fiscal_year);

CREATE TABLE IF NOT EXISTS purchase_orders (
    id            BIGINT      PRIMARY KEY,
    vendor_id     BIGINT      NOT NULL REFERENCES vendors(id),
    fund_id       BIGINT      REFERENCES acquisition_funds(id) ON DELETE SET NULL,
    order_number  VARCHAR(50) NOT NULL,
    status        VARCHAR(20) NOT NULL DEFAULT 'draft',
    notes         TEXT,
    ordered_at    TIMESTAMPTZ,
    created_by    BIGINT      REFERENCES users(id) ON DELETE SET NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (order_number)
);

CREATE INDEX IF NOT EXISTS idx_purchase_orders_vendor ON purchase_orders (vendor_id);
CREATE INDEX IF NOT EXISTS idx_purchase_orders_fund ON purchase_orders (fund_id);
CREATE INDEX IF NOT EXISTS idx_purchase_orders_status ON purchase_orders (status);

CREATE TABLE IF NOT EXISTS purchase_order_lines (
    id                 BIGINT         PRIMARY KEY,
    purchase_order_id  BIGINT         NOT NULL REFERENCES purchase_orders(id) ON DELETE CASCADE,
    fund_id            BIGINT         REFERENCES acquisition_funds(id) ON DELETE SET NULL,
    biblio_id          BIGINT         REFERENCES biblios(id) ON DELETE SET NULL,
    isbn               VARCHAR(30),
    title              VARCHAR(500),
    quantity_ordered   INTEGER        NOT NULL,
    quantity_received  INTEGER        NOT NULL DEFAULT 0,
    unit_price         NUMERIC(14, 2),
    currency           VARCHAR(3)     NOT NULL DEFAULT 'EUR',
    notes              TEXT,
    created_at         TIMESTAMPTZ    NOT NULL DEFAULT NOW(),
    updated_at         TIMESTAMPTZ    NOT NULL DEFAULT NOW(),
    CONSTRAINT purchase_order_lines_qty_ordered_chk CHECK (quantity_ordered > 0),
    CONSTRAINT purchase_order_lines_qty_received_chk CHECK (quantity_received >= 0),
    CONSTRAINT purchase_order_lines_qty_le_ordered_chk CHECK (quantity_received <= quantity_ordered)
);

CREATE INDEX IF NOT EXISTS idx_po_lines_order ON purchase_order_lines (purchase_order_id);
CREATE INDEX IF NOT EXISTS idx_po_lines_fund ON purchase_order_lines (fund_id);
CREATE INDEX IF NOT EXISTS idx_po_lines_biblio ON purchase_order_lines (biblio_id);

CREATE TABLE IF NOT EXISTS receipts (
    id                 BIGINT      PRIMARY KEY,
    purchase_order_id  BIGINT      NOT NULL REFERENCES purchase_orders(id) ON DELETE CASCADE,
    received_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    received_by        BIGINT      REFERENCES users(id) ON DELETE SET NULL,
    notes              TEXT,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_receipts_order ON receipts (purchase_order_id);

CREATE TABLE IF NOT EXISTS receipt_lines (
    id                      BIGINT         PRIMARY KEY,
    receipt_id              BIGINT         NOT NULL REFERENCES receipts(id) ON DELETE CASCADE,
    purchase_order_line_id  BIGINT         NOT NULL REFERENCES purchase_order_lines(id),
    quantity                INTEGER        NOT NULL,
    unit_price              NUMERIC(14, 2),
    created_at              TIMESTAMPTZ    NOT NULL DEFAULT NOW(),
    CONSTRAINT receipt_lines_qty_chk CHECK (quantity > 0)
);

CREATE INDEX IF NOT EXISTS idx_receipt_lines_receipt ON receipt_lines (receipt_id);
CREATE INDEX IF NOT EXISTS idx_receipt_lines_po_line ON receipt_lines (purchase_order_line_id);

CREATE TABLE IF NOT EXISTS receipt_items (
    receipt_line_id  BIGINT NOT NULL REFERENCES receipt_lines(id) ON DELETE CASCADE,
    item_id          BIGINT NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    PRIMARY KEY (receipt_line_id, item_id)
);

CREATE INDEX IF NOT EXISTS idx_receipt_items_item ON receipt_items (item_id);
