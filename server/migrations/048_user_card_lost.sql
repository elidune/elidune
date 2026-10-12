-- Library card declared lost or stolen. Checkout refuses the card unless force=true.
-- Distinct from items.circulation_status (a lost copy) and users.status = 'blocked'.

ALTER TABLE users
    ADD COLUMN IF NOT EXISTS card_lost BOOLEAN NOT NULL DEFAULT FALSE;

COMMENT ON COLUMN users.card_lost IS
    'True when this patron''s library card is declared lost or stolen. Checkout refuses unless force=true.';
