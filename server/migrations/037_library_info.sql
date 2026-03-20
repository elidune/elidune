-- Migration 037: Library information table

CREATE TABLE IF NOT EXISTS library_info (
    id           SMALLINT PRIMARY KEY DEFAULT 1,
    name         TEXT,
    addr_line1   VARCHAR(100),
    addr_line2   VARCHAR(100),
    addr_postcode VARCHAR(10),
    addr_city    VARCHAR(100),
    addr_country VARCHAR(50),
    phones       JSONB NOT NULL DEFAULT '[]'::jsonb,
    email        TEXT,
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT library_info_single_row CHECK (id = 1)
);

INSERT INTO library_info (id) VALUES (1) ON CONFLICT DO NOTHING;
