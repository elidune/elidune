-- Events target zero or more audiences via event_audiences.
-- Legacy events.public_type (single public_types.name, NULL = everyone) is copied:
--   events.all_audiences = true when public_type IS NULL (no link rows)
--   one event_audiences row otherwise.
-- The legacy column and its FK stay in place. It is nullable, so inserts that omit it
-- still succeed. The application no longer reads or writes it. A later migration drops
-- it after the main -> dev merge settles. This file stays additive so a rollback of the
-- audience feature does not depend on recreating the column.
-- An empty audience list is not stored: all audiences is the boolean, not "no rows" by itself.

ALTER TABLE events
    ADD COLUMN IF NOT EXISTS all_audiences BOOLEAN NOT NULL DEFAULT FALSE;

CREATE TABLE IF NOT EXISTS event_audiences (
    event_id  BIGINT       NOT NULL REFERENCES events(id) ON DELETE CASCADE,
    audience  VARCHAR(50)  NOT NULL REFERENCES public_types(name) ON UPDATE CASCADE,
    PRIMARY KEY (event_id, audience)
);

CREATE INDEX IF NOT EXISTS idx_event_audiences_audience ON event_audiences(audience);

-- BEGIN LEGACY AUDIENCE COPY
INSERT INTO event_audiences (event_id, audience)
SELECT e.id, e.public_type
FROM events e
WHERE e.public_type IS NOT NULL
ON CONFLICT (event_id, audience) DO NOTHING;

UPDATE events
SET all_audiences = TRUE
WHERE public_type IS NULL;
-- END LEGACY AUDIENCE COPY

COMMENT ON COLUMN events.all_audiences IS
    'True when the event targets every audience. event_audiences is empty in that case. False requires at least one event_audiences row.';
COMMENT ON TABLE event_audiences IS
    'Target audiences for an event (public_types.name). Empty only when events.all_audiences is true.';
COMMENT ON COLUMN events.public_type IS
    'Legacy single audience (public_types.name), nullable. Retained unused so this migration stays reversible. The application reads and writes all_audiences and event_audiences instead.';
