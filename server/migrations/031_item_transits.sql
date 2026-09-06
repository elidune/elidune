-- Inter-site item transit for hold fulfillment (#15).
-- Transfer is item movement + the existing hold queue, not a second reservation type.
-- Pickup site on holds now references sources (sites).

ALTER TABLE holds
    ADD CONSTRAINT holds_pickup_site_id_fkey
        FOREIGN KEY (pickup_site_id) REFERENCES sources (id) ON DELETE SET NULL;

COMMENT ON COLUMN holds.pickup_site_id IS
    'Pickup site (sources.id). When set and different from the allocated copy''s current source, fulfillment goes through item transit.';

CREATE TABLE IF NOT EXISTS item_transits (
    id               BIGINT PRIMARY KEY,
    item_id          BIGINT NOT NULL REFERENCES items (id) ON DELETE CASCADE,
    hold_id          BIGINT REFERENCES holds (id) ON DELETE SET NULL,
    from_source_id   BIGINT NOT NULL REFERENCES sources (id),
    to_source_id     BIGINT NOT NULL REFERENCES sources (id),
    status           VARCHAR(20) NOT NULL DEFAULT 'requested',
    notes            TEXT,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    shipped_at       TIMESTAMPTZ,
    received_at      TIMESTAMPTZ,
    cancelled_at     TIMESTAMPTZ,
    shipped_by       BIGINT REFERENCES users (id) ON DELETE SET NULL,
    received_by      BIGINT REFERENCES users (id) ON DELETE SET NULL,
    cancelled_by     BIGINT REFERENCES users (id) ON DELETE SET NULL,
    reversed_from_id BIGINT REFERENCES item_transits (id) ON DELETE SET NULL,
    CONSTRAINT item_transits_status_check
        CHECK (status IN ('requested', 'in_transit', 'received', 'cancelled')),
    CONSTRAINT item_transits_sites_differ
        CHECK (from_source_id <> to_source_id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_item_transits_one_active_per_item
    ON item_transits (item_id)
    WHERE status IN ('requested', 'in_transit');

CREATE UNIQUE INDEX IF NOT EXISTS idx_item_transits_one_active_per_hold
    ON item_transits (hold_id)
    WHERE hold_id IS NOT NULL AND status IN ('requested', 'in_transit');

CREATE INDEX IF NOT EXISTS idx_item_transits_hold ON item_transits (hold_id);
CREATE INDEX IF NOT EXISTS idx_item_transits_status ON item_transits (status);
CREATE INDEX IF NOT EXISTS idx_item_transits_to_source
    ON item_transits (to_source_id)
    WHERE status IN ('requested', 'in_transit');

COMMENT ON TABLE item_transits IS
    'Inter-site item movement for hold fulfillment. Not a reservation type; the hold queue remains the reservation.';
COMMENT ON COLUMN item_transits.status IS
    'requested → in_transit → received. cancelled frees the copy; a reverse row may be created.';
