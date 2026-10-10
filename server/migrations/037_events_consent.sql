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
-- A child is public_types.name = 'child'. Birthdate is not used.
-- NULL public type and non-child public types (adult, school, staff, senior) are included.
-- The public-type name "staff" is not an account type and stays included.
-- Staff logins are excluded: users.account_type references account_types.code,
-- and codes 'librarian' and 'admin' are not opted in. They can opt in later from their profile.
-- A subscription that ended more than one year ago is excluded
-- (expiry_at IS NOT NULL AND expiry_at < NOW() - INTERVAL '1 year').
-- NULL expiry_at stays included. An expiry less than one year ago stays included.
-- The overdue-reminder preference is intentionally not consulted.
UPDATE users AS u
SET
    events_consent_at = NOW(),
    events_consent_source = 'migration',
    events_consent_changed_at = NOW()
WHERE (u.status IS NULL OR u.status <> 'deleted')
  AND u.account_type NOT IN ('librarian', 'admin')
  AND NOT (
      u.expiry_at IS NOT NULL
      AND u.expiry_at < NOW() - INTERVAL '1 year'
  )
  AND NOT EXISTS (
      SELECT 1
      FROM public_types AS pt
      WHERE pt.id = u.public_type
        AND pt.name = 'child'
  );
-- END EVENTS CONSENT BACKFILL
