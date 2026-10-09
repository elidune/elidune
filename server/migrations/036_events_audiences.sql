-- Events target zero or more audiences via event_audiences.
-- Legacy events.public_type (single public_types.name, NULL = everyone) becomes:
--   events.all_audiences = true when public_type IS NULL (no link rows)
--   one event_audiences row otherwise.
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

ALTER TABLE events DROP CONSTRAINT IF EXISTS events_public_type_name_fkey;
ALTER TABLE events DROP COLUMN IF EXISTS public_type;

COMMENT ON COLUMN events.all_audiences IS
    'True when the event targets every audience. event_audiences is empty in that case. False requires at least one event_audiences row.';
COMMENT ON TABLE event_audiences IS
    'Target audiences for an event (public_types.name). Empty only when events.all_audiences is true.';
