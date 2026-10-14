-- German and Spanish notice templates.
-- A reader email is entirely in one language: the reader's language when that
-- template exists, otherwise the library language (fr, en, de, es), otherwise French.

ALTER TABLE email_templates
    DROP CONSTRAINT IF EXISTS email_templates_language_chk;

ALTER TABLE email_templates
    ADD CONSTRAINT email_templates_language_chk
    CHECK (language IN ('english', 'french', 'german', 'spanish'));

COMMENT ON COLUMN email_templates.language IS
    'Language key matching models::Language (english, french, german, spanish).';
