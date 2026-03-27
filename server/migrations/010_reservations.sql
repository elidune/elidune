-- Holds / reservations queue per physical item (items.id).

CREATE TABLE reservations (
    id           BIGINT       PRIMARY KEY,
    user_id      BIGINT       NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    item_id      BIGINT       NOT NULL REFERENCES items (id) ON DELETE CASCADE,
    created_at   TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    notified_at  TIMESTAMPTZ,
    expires_at   TIMESTAMPTZ,
    status       VARCHAR(32)  NOT NULL DEFAULT 'pending',
    position     INTEGER      NOT NULL DEFAULT 1,
    notes        TEXT
);

CREATE INDEX idx_reservations_user_id ON reservations (user_id);
CREATE INDEX idx_reservations_item_id ON reservations (item_id);
CREATE INDEX idx_reservations_item_status ON reservations (item_id, status);
