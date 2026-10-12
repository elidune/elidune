-- Card numbers removed when a library card is replaced.
-- A number stored here must never be assigned to any user again
-- (create, update, or a later replacement).

CREATE TABLE IF NOT EXISTS retired_card_numbers (
    id           BIGINT PRIMARY KEY,
    card_number  VARCHAR(100) NOT NULL,
    user_id      BIGINT NOT NULL REFERENCES users (id),
    retired_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    retired_by   BIGINT REFERENCES users (id),
    CONSTRAINT retired_card_numbers_card_number_key UNIQUE (card_number)
);

CREATE INDEX IF NOT EXISTS idx_retired_card_numbers_user_id
    ON retired_card_numbers (user_id);

COMMENT ON TABLE retired_card_numbers IS
    'Library card numbers taken off a patron by card replacement. Never assign these values again.';

COMMENT ON COLUMN retired_card_numbers.card_number IS
    'Exact previous users.barcode. Comparison is case-sensitive, matching users.barcode.';

COMMENT ON COLUMN retired_card_numbers.retired_by IS
    'Staff user who replaced the card. Null only if that account is removed later.';
