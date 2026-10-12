-- Library card declared lost or stolen. Checkout always refuses (422 card_lost).
-- force does not override. Distinct from items.circulation_status (a lost copy)
-- and users.status = 'blocked'.

ALTER TABLE users
    ADD COLUMN IF NOT EXISTS card_lost BOOLEAN NOT NULL DEFAULT FALSE;

COMMENT ON COLUMN users.card_lost IS
    'True when this patron''s library card is declared lost or stolen. Checkout always refuses with 422 card_lost; force does not override.';
