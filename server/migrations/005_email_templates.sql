-- Persisted email templates so administrators can edit subject / plain / HTML bodies at runtime.
-- Initial bootstrap (rows seeded from data/email_templates/*.json) is performed by the server at
-- startup when the table is empty (see crate::email_templates::bootstrap_from_files).

CREATE TABLE IF NOT EXISTS email_templates (
    template_id  VARCHAR(64) NOT NULL,
    language     VARCHAR(16) NOT NULL,
    subject      TEXT        NOT NULL,
    body_plain   TEXT        NOT NULL,
    body_html    TEXT,
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (template_id, language),
    CONSTRAINT email_templates_language_chk CHECK (language IN ('english', 'french'))
);

COMMENT ON TABLE  email_templates              IS 'Editable email templates. Populated from data/email_templates/*.json on first startup.';
COMMENT ON COLUMN email_templates.template_id  IS 'Logical id: 2fa_code, recovery_code, password_reset, hold_ready, overdue_reminder, event_announcement.';
COMMENT ON COLUMN email_templates.language     IS 'Language key matching models::Language (english, french).';
