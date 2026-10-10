-- Dedicated opt-in for event announcements, separate from receive_reminders.
-- NULL events_consent_at means no consent. Source is stamped by the server.

ALTER TABLE users
    ADD COLUMN IF NOT EXISTS events_consent_at TIMESTAMPTZ NULL,
    ADD COLUMN IF NOT EXISTS events_consent_source VARCHAR(20) NULL,
    ADD COLUMN IF NOT EXISTS events_consent_changed_at TIMESTAMPTZ NULL,
    ADD COLUMN IF NOT EXISTS events_consent_notice_at TIMESTAMPTZ NULL;

ALTER TABLE users
    DROP CONSTRAINT IF EXISTS users_events_consent_source_check;

ALTER TABLE users
    ADD CONSTRAINT users_events_consent_source_check
    CHECK (
        events_consent_source IS NULL
        OR events_consent_source IN (
            'registration',
            'desk',
            'profile',
            'migration',
            'unsubscribe'
        )
    );

-- BEGIN EVENTS CONSENT BACKFILL
-- Active = not deleted (same predicate as announcement recipient selection).
-- Adult = public type is not the seeded `child` name. Birthdate is not used.
-- NULL public type and non-child types (adult, school, staff, senior) are included.
-- receive_reminders is intentionally not consulted.
UPDATE users AS u
SET
    events_consent_at = NOW(),
    events_consent_source = 'migration',
    events_consent_changed_at = NOW()
WHERE (u.status IS NULL OR u.status <> 'deleted')
  AND NOT EXISTS (
      SELECT 1
      FROM public_types AS pt
      WHERE pt.id = u.public_type
        AND pt.name = 'child'
  );
-- END EVENTS CONSENT BACKFILL
