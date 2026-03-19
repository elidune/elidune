-- =============================================================================
-- Migration 033: Public types table and user public_type FK migration
-- =============================================================================
-- Creates public_types table with default types (child, adult, school, staff, senior),
-- public_type_loan_settings for per-public-type media overrides,
-- and migrates users.public_type from magic integers (97, 106, 117) to FK references.

-- A. Create public_types table
CREATE TABLE public_types (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(50) NOT NULL UNIQUE,
    label VARCHAR(100) NOT NULL,
    subscription_duration_days INTEGER DEFAULT 365,
    age_min SMALLINT,
    age_max SMALLINT,
    subscription_price INTEGER DEFAULT 0,
    max_loans SMALLINT,
    loan_duration_days SMALLINT
);

-- B. Create public_type_loan_settings table (per-media-type overrides per public type)
CREATE TABLE public_type_loan_settings (
    id BIGSERIAL PRIMARY KEY,
    public_type_id BIGINT NOT NULL REFERENCES public_types(id) ON DELETE CASCADE,
    media_type VARCHAR(50) NOT NULL,
    duration SMALLINT,
    nb_max SMALLINT,
    nb_renews SMALLINT,
    UNIQUE(public_type_id, media_type)
);

CREATE INDEX idx_public_type_loan_settings_public_type ON public_type_loan_settings(public_type_id);

-- C. Insert default public types (IDs will be 1, 2, 3, 4, 5)
INSERT INTO public_types (name, label, subscription_duration_days, age_min, age_max, subscription_price, max_loans, loan_duration_days)
VALUES
    ('child', 'Enfant', 365, 0, 12, 0, 10, 21),
    ('adult', 'Adulte', 365, 18, 99, 1500, 5, 21),
    ('school', 'École', 365, NULL, NULL, 0, 50, 60),
    ('staff', 'Personnel', 365, NULL, NULL, NULL, NULL, NULL),
    ('senior', 'Senior', 365, 60, 99, 0, 5, 21);

-- D. Migrate users.public_type from INTEGER (97, 106, 117) to BIGINT FK
-- Drop FK if it exists (e.g. from a previous partial migration)
ALTER TABLE users DROP CONSTRAINT IF EXISTS fk_users_public_type;

-- Change column type to BIGINT (97/106/117 fit in bigint)
ALTER TABLE users ALTER COLUMN public_type TYPE BIGINT USING public_type::bigint;

-- Map old values to new public_type IDs (child=1, adult=2, school=3, staff=4, senior=5)
UPDATE users SET public_type = (SELECT id FROM public_types WHERE name = 'adult') WHERE public_type = 97;
UPDATE users SET public_type = (SELECT id FROM public_types WHERE name = 'child') WHERE public_type = 106;
UPDATE users SET public_type = (SELECT id FROM public_types WHERE name = 'senior') WHERE public_type = 117;

-- Set any unmapped legacy values to NULL
UPDATE users SET public_type = NULL
WHERE public_type IS NOT NULL AND public_type NOT IN (SELECT id FROM public_types);

-- Add FK constraint
ALTER TABLE users ADD CONSTRAINT fk_users_public_type
    FOREIGN KEY (public_type) REFERENCES public_types(id);

-- E. Migrate loans_archives.borrower_public_type (same mapping)
ALTER TABLE loans_archives DROP CONSTRAINT IF EXISTS fk_loans_archives_borrower_public_type;

ALTER TABLE loans_archives ALTER COLUMN borrower_public_type TYPE BIGINT USING borrower_public_type::bigint;

UPDATE loans_archives SET borrower_public_type = (SELECT id FROM public_types WHERE name = 'adult') WHERE borrower_public_type = 97;
UPDATE loans_archives SET borrower_public_type = (SELECT id FROM public_types WHERE name = 'child') WHERE borrower_public_type = 106;
UPDATE loans_archives SET borrower_public_type = (SELECT id FROM public_types WHERE name = 'senior') WHERE borrower_public_type = 117;

-- Set any unmapped legacy values to NULL
UPDATE loans_archives SET borrower_public_type = NULL
WHERE borrower_public_type IS NOT NULL AND borrower_public_type NOT IN (SELECT id FROM public_types);

-- Add FK on loans_archives
ALTER TABLE loans_archives ADD CONSTRAINT fk_loans_archives_borrower_public_type
    FOREIGN KEY (borrower_public_type) REFERENCES public_types(id) ON DELETE SET NULL;
