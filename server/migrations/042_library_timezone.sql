-- Library IANA timezone for patron-facing dates and the opening calendar.
-- Site closures: schedule_closures.source_id NULL = network-wide; set = that site only.

ALTER TABLE library_info
    ADD COLUMN IF NOT EXISTS timezone TEXT NOT NULL DEFAULT 'Europe/Paris';

COMMENT ON COLUMN library_info.timezone IS
    'IANA timezone name (for example Europe/Paris) used for patron-facing dates and open-day calculations.';

ALTER TABLE schedule_closures
    ADD COLUMN IF NOT EXISTS source_id BIGINT REFERENCES sources(id) ON DELETE CASCADE;

COMMENT ON COLUMN schedule_closures.source_id IS
    'NULL = network-wide closure (every site). Set = this site only (sources.id).';

CREATE INDEX IF NOT EXISTS idx_schedule_closures_source_date
    ON schedule_closures (source_id, closure_date);
