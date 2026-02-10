-- =============================================================================
-- Elidune Database Initialization Script
-- =============================================================================
-- This script creates all tables and populates default data for a fresh database
-- Run this script on an empty database to set up Elidune from scratch
--
-- Usage:
--   psql -U elidune -d elidune -f scripts/init_database.sql
-- =============================================================================

-- =============================================================================
-- REFERENCE TABLES
-- =============================================================================

-- Account types table (code as primary key)
CREATE TABLE IF NOT EXISTS account_types (
    code VARCHAR(50) PRIMARY KEY,
    name VARCHAR,
    items_rights CHAR(1) DEFAULT 'n',
    users_rights CHAR(1) DEFAULT 'n',
    loans_rights CHAR(1) DEFAULT 'n',
    items_archive_rights CHAR(1) DEFAULT 'n',
    borrows_rights CHAR(1),
    settings_rights CHAR(1)
);

-- Fees table (code as primary key)
CREATE TABLE IF NOT EXISTS fees (
    code VARCHAR(50) PRIMARY KEY,
    name VARCHAR,
    amount INTEGER DEFAULT 0
);

-- =============================================================================
-- USERS TABLE
-- =============================================================================

CREATE TABLE IF NOT EXISTS users (
    id SERIAL PRIMARY KEY,
    login VARCHAR NOT NULL,
    password VARCHAR(255),
    firstname VARCHAR,
    lastname VARCHAR,
    email VARCHAR,
    addr_street VARCHAR,
    addr_zip_code INTEGER,
    addr_city VARCHAR,
    phone VARCHAR,
    birthdate VARCHAR,
    account_type VARCHAR(50) NOT NULL,
    fee VARCHAR(50),
    group_id INTEGER,
    barcode VARCHAR,
    notes VARCHAR,
    public_type INTEGER,
    status SMALLINT DEFAULT 0,
    crea_date TIMESTAMPTZ,
    modif_date TIMESTAMPTZ,
    issue_date TIMESTAMPTZ,
    archived_date TIMESTAMPTZ,
    language VARCHAR(5) DEFAULT 'fr',
    -- 2FA fields
    two_factor_enabled BOOLEAN DEFAULT FALSE,
    two_factor_method VARCHAR(10) CHECK (two_factor_method IN ('totp', 'email', NULL)),
    totp_secret VARCHAR(255),
    recovery_codes TEXT,
    recovery_codes_used TEXT DEFAULT '[]',
    CONSTRAINT users_login_unique UNIQUE (login)
);

CREATE INDEX IF NOT EXISTS users_id_key ON users (id);
CREATE INDEX IF NOT EXISTS users_login_key ON users (login);
CREATE INDEX IF NOT EXISTS users_barcode_key ON users (barcode);
CREATE INDEX IF NOT EXISTS users_account_type_idx ON users(account_type);
CREATE INDEX IF NOT EXISTS users_fee_idx ON users(fee);
CREATE INDEX IF NOT EXISTS users_email_idx ON users (email) WHERE email IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_users_status ON users(status);
CREATE INDEX IF NOT EXISTS idx_users_two_factor_enabled ON users(two_factor_enabled);

-- =============================================================================
-- CATALOG TABLES
-- =============================================================================

-- Authors table
CREATE TABLE IF NOT EXISTS authors (
    id SERIAL PRIMARY KEY,
    key VARCHAR UNIQUE,
    lastname VARCHAR,
    firstname VARCHAR,
    bio VARCHAR,
    notes VARCHAR
);

CREATE INDEX IF NOT EXISTS authors_id_key ON authors (id);
CREATE INDEX IF NOT EXISTS authors_lastname_key ON authors (lastname);

-- Editions table
CREATE TABLE IF NOT EXISTS editions (
    id SERIAL PRIMARY KEY,
    key VARCHAR,
    name VARCHAR,
    place VARCHAR,
    notes VARCHAR
);

CREATE INDEX IF NOT EXISTS editions_id_key ON editions (id);
CREATE INDEX IF NOT EXISTS editions_name_key ON editions (name);

-- Collections table
CREATE TABLE IF NOT EXISTS collections (
    id SERIAL PRIMARY KEY,
    key VARCHAR,
    title1 VARCHAR,
    title2 VARCHAR,
    title3 VARCHAR,
    issn VARCHAR
);

CREATE INDEX IF NOT EXISTS collections_id_key ON collections (id);

-- Series table
CREATE TABLE IF NOT EXISTS series (
    id SERIAL PRIMARY KEY,
    key VARCHAR,
    name VARCHAR
);

CREATE INDEX IF NOT EXISTS series_id_key ON series (id);
CREATE INDEX IF NOT EXISTS series_name_key ON series (name);

-- Sources table
CREATE TABLE IF NOT EXISTS sources (
    id SERIAL PRIMARY KEY,
    key VARCHAR,
    name VARCHAR
);

CREATE INDEX IF NOT EXISTS sources_id_key ON sources (id);
CREATE INDEX IF NOT EXISTS sources_name_key ON sources (name);

-- Items table
CREATE TABLE IF NOT EXISTS items (
    id SERIAL PRIMARY KEY,
    media_type VARCHAR,
    identification VARCHAR,
    price VARCHAR,
    barcode VARCHAR,
    dewey VARCHAR,
    publication_date VARCHAR,
    lang SMALLINT,
    lang_orig SMALLINT,
    title1 VARCHAR,
    title2 VARCHAR,
    title3 VARCHAR,
    title4 VARCHAR,
    author1_ids INTEGER[],
    author1_functions VARCHAR,
    author2_ids INTEGER[],
    author2_functions VARCHAR,
    author3_ids INTEGER[],
    author3_functions VARCHAR,
    serie_id INTEGER,
    serie_vol_number SMALLINT,
    collection_id INTEGER,
    collection_number_sub SMALLINT,
    collection_vol_number SMALLINT,
    source_id INTEGER,
    genre SMALLINT,
    subject VARCHAR,
    public_type SMALLINT,
    edition_id INTEGER,
    edition_date VARCHAR,
    nb_pages VARCHAR,
    format VARCHAR,
    content VARCHAR,
    addon VARCHAR,
    abstract VARCHAR,
    notes VARCHAR,
    keywords VARCHAR,
    nb_specimens SMALLINT,
    state VARCHAR,
    is_archive SMALLINT DEFAULT 0,
    archived_timestamp TIMESTAMPTZ,
    is_valid SMALLINT DEFAULT 0,
    crea_date TIMESTAMPTZ,
    modif_date TIMESTAMPTZ,
    lifecycle_status SMALLINT DEFAULT 0 NOT NULL,
    archived_date TIMESTAMPTZ,
    search_vector tsvector
);

CREATE INDEX IF NOT EXISTS items_id_key ON items (id);
CREATE INDEX IF NOT EXISTS items_identification_key ON items (identification);
CREATE INDEX IF NOT EXISTS items_title1_key ON items (title1);
CREATE INDEX IF NOT EXISTS items_search_vector_idx ON items USING GIN(search_vector);
CREATE INDEX IF NOT EXISTS items_lifecycle_status_idx ON items (lifecycle_status);

-- Full-text search trigger function
CREATE OR REPLACE FUNCTION items_search_vector_update() RETURNS trigger AS $$
BEGIN
    NEW.search_vector := 
        setweight(to_tsvector('french', COALESCE(NEW.title1, '')), 'A') ||
        setweight(to_tsvector('french', COALESCE(NEW.title2, '')), 'B') ||
        setweight(to_tsvector('french', COALESCE(NEW.keywords, '')), 'B') ||
        setweight(to_tsvector('french', COALESCE(NEW.abstract, '')), 'C') ||
        setweight(to_tsvector('french', COALESCE(NEW.subject, '')), 'C') ||
        setweight(to_tsvector('french', COALESCE(NEW.content, '')), 'D');
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS items_search_vector_trigger ON items;
CREATE TRIGGER items_search_vector_trigger
    BEFORE INSERT OR UPDATE ON items
    FOR EACH ROW
    EXECUTE FUNCTION items_search_vector_update();

-- Remote items table
CREATE TABLE IF NOT EXISTS remote_items (
    id SERIAL PRIMARY KEY,
    media_type VARCHAR,
    identification VARCHAR,
    price VARCHAR,
    barcode VARCHAR,
    dewey VARCHAR,
    publication_date VARCHAR,
    lang SMALLINT,
    lang_orig SMALLINT,
    title1 VARCHAR,
    title2 VARCHAR,
    title3 VARCHAR,
    title4 VARCHAR,
    author1_ids INTEGER[],
    author1_functions VARCHAR,
    author2_ids INTEGER[],
    author2_functions VARCHAR,
    author3_ids INTEGER[],
    author3_functions VARCHAR,
    serie_id INTEGER,
    serie_vol_number SMALLINT,
    collection_id INTEGER,
    collection_number_sub SMALLINT,
    collection_vol_number SMALLINT,
    source_id INTEGER,
    source_date VARCHAR,
    source_norme VARCHAR,
    genre SMALLINT,
    subject VARCHAR,
    public_type SMALLINT,
    edition_id INTEGER,
    edition_date VARCHAR,
    nb_pages VARCHAR,
    format VARCHAR,
    content VARCHAR,
    addon VARCHAR,
    abstract VARCHAR,
    notes VARCHAR,
    keywords VARCHAR,
    nb_specimens SMALLINT,
    state VARCHAR,
    is_archive SMALLINT DEFAULT 0,
    archived_timestamp TIMESTAMPTZ,
    is_valid SMALLINT DEFAULT 0,
    modif_date TIMESTAMPTZ,
    crea_date TIMESTAMPTZ,
    lifecycle_status SMALLINT DEFAULT 0 NOT NULL,
    -- JSON columns for authors
    authors1_json JSONB,
    authors2_json JSONB,
    authors3_json JSONB
);

CREATE INDEX IF NOT EXISTS remote_items_id_key ON remote_items (id);
CREATE INDEX IF NOT EXISTS remote_items_identification_key ON remote_items (identification);
CREATE INDEX IF NOT EXISTS remote_items_title1_key ON remote_items (title1);
CREATE INDEX IF NOT EXISTS idx_remote_items_authors1_json ON remote_items USING GIN (authors1_json);
CREATE INDEX IF NOT EXISTS idx_remote_items_authors2_json ON remote_items USING GIN (authors2_json);
CREATE INDEX IF NOT EXISTS idx_remote_items_authors3_json ON remote_items USING GIN (authors3_json);

-- Specimens table
CREATE TABLE IF NOT EXISTS specimens (
    id SERIAL PRIMARY KEY,
    id_item INTEGER,
    source_id INTEGER,
    identification VARCHAR,
    cote VARCHAR,
    place SMALLINT,
    status SMALLINT,
    codestat SMALLINT,
    notes VARCHAR,
    price VARCHAR,
    modif_date TIMESTAMPTZ,
    is_archive INTEGER DEFAULT 0,
    archive_date TIMESTAMPTZ,
    crea_date TIMESTAMPTZ,
    lifecycle_status SMALLINT DEFAULT 0 NOT NULL
);

CREATE INDEX IF NOT EXISTS specimens_id_key ON specimens (id);
CREATE INDEX IF NOT EXISTS specimens_id_item_key ON specimens (id_item);
CREATE INDEX IF NOT EXISTS specimens_source_id_key ON specimens (source_id);
CREATE INDEX IF NOT EXISTS specimens_identification_key ON specimens (identification);
CREATE INDEX IF NOT EXISTS specimens_lifecycle_status_idx ON specimens (lifecycle_status);

-- Remote specimens table
CREATE TABLE IF NOT EXISTS remote_specimens (
    id SERIAL PRIMARY KEY,
    id_item INTEGER,
    source_id INTEGER,
    identification VARCHAR,
    cote VARCHAR,
    media_type VARCHAR,
    place SMALLINT,
    status SMALLINT,
    codestat SMALLINT,
    notes VARCHAR,
    price VARCHAR,
    creation_date TIMESTAMPTZ,
    modif_date TIMESTAMPTZ,
    lifecycle_status SMALLINT DEFAULT 0 NOT NULL
);

CREATE INDEX IF NOT EXISTS remote_specimens_id_key ON remote_specimens (id);
CREATE INDEX IF NOT EXISTS remote_specimens_id_item_key ON remote_specimens (id_item);
CREATE INDEX IF NOT EXISTS remote_specimens_identification_key ON remote_specimens (identification);

-- =============================================================================
-- LOANS TABLES
-- =============================================================================

-- Loans table (active loans)
CREATE TABLE IF NOT EXISTS loans (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL,
    specimen_id INTEGER NOT NULL,
    item_id INTEGER,
    date TIMESTAMPTZ NOT NULL,
    renew_date TIMESTAMPTZ,
    nb_renews SMALLINT,
    issue_date TIMESTAMPTZ,
    notes VARCHAR,
    returned_date TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS loans_id_key ON loans (id);
CREATE INDEX IF NOT EXISTS loans_user_id_key ON loans (user_id);
CREATE INDEX IF NOT EXISTS loans_specimen_id_key ON loans (specimen_id);

-- Loans archives table
CREATE TABLE IF NOT EXISTS loans_archives (
    id SERIAL PRIMARY KEY,
    user_id INTEGER,
    item_id INTEGER NOT NULL,
    specimen_id INTEGER,
    date TIMESTAMPTZ NOT NULL,
    nb_renews SMALLINT,
    issue_date TIMESTAMPTZ,
    returned_date TIMESTAMPTZ,
    notes VARCHAR,
    borrower_public_type INTEGER,
    addr_city VARCHAR,
    account_type VARCHAR(50)
);

CREATE INDEX IF NOT EXISTS loans_archives_id_key ON loans_archives (id);
CREATE INDEX IF NOT EXISTS loans_archives_item_id_key ON loans_archives (item_id);

-- Loans settings table
CREATE TABLE IF NOT EXISTS loans_settings (
    id SERIAL PRIMARY KEY,
    media_type VARCHAR UNIQUE,
    nb_max SMALLINT,
    nb_renews SMALLINT,
    duration SMALLINT,
    notes VARCHAR,
    account_type VARCHAR(50)
);

CREATE INDEX IF NOT EXISTS loans_settings_id_key ON loans_settings (id);
CREATE INDEX IF NOT EXISTS loans_settings_media_type_key ON loans_settings (media_type);

-- =============================================================================
-- Z39.50 SERVERS TABLE
-- =============================================================================

CREATE TABLE IF NOT EXISTS z3950servers (
    id SERIAL PRIMARY KEY,
    address VARCHAR,
    port INTEGER DEFAULT 2200 NOT NULL,
    name VARCHAR,
    description VARCHAR,
    activated INTEGER,
    login VARCHAR,
    password VARCHAR,
    database VARCHAR,
    format VARCHAR,
    encoding VARCHAR DEFAULT 'utf-8' NOT NULL
);

-- =============================================================================
-- DEFAULT DATA
-- =============================================================================

-- Account types
INSERT INTO account_types (code, name, items_rights, users_rights, loans_rights, items_archive_rights, borrows_rights, settings_rights) VALUES
('guest', 'Guest', 'r', 'r', 'n', 'n', 'n', 'r'),
('reader', 'Reader', 'r', 'r', 'r', 'r', 'r', 'r'),
('librarian', 'Librarian', 'w', 'w', 'w', 'w', 'w', 'r'),
('admin', 'Administrator', 'w', 'w', 'w', 'w', 'w', 'w'),
('group', 'Group', 'r', 'r', 'r', 'r', 'r', 'r')
ON CONFLICT (code) DO UPDATE SET
    name = EXCLUDED.name,
    items_rights = EXCLUDED.items_rights,
    users_rights = EXCLUDED.users_rights,
    loans_rights = EXCLUDED.loans_rights,
    items_archive_rights = EXCLUDED.items_archive_rights,
    borrows_rights = EXCLUDED.borrows_rights,
    settings_rights = EXCLUDED.settings_rights;

-- Fees
INSERT INTO fees (code, name, amount) VALUES
('free', 'Free', 0),
('local', 'Local', 5),
('foreigner', 'Foreigner', 10)
ON CONFLICT (code) DO UPDATE SET
    name = EXCLUDED.name,
    amount = EXCLUDED.amount;

-- Z39.50 servers
INSERT INTO z3950servers (id, address, port, name, description, activated, login, password, database, format, encoding) VALUES
(1, 'catalogue.bnf.fr', 2200, 'BNF', 'Bibliothèque nationale de France', 1, NULL, NULL, 'TOUT-UTF8', 'UNIMARC', 'utf-8'),
(2, 'opac.sudoc.abes.fr', 2200, 'SUDOC', 'Système Universitaire de Documentation', 1, NULL, NULL, 'abes', 'UNIMARC', 'utf-8'),
(3, 'z3950.loc.gov', 7090, 'Library of Congress', 'Library of Congress Z39.50 Server', 1, NULL, NULL, 'VOYAGER', 'MARC21', 'utf-8')
ON CONFLICT (id) DO UPDATE SET
    address = EXCLUDED.address,
    port = EXCLUDED.port,
    name = EXCLUDED.name,
    description = EXCLUDED.description,
    activated = EXCLUDED.activated,
    login = EXCLUDED.login,
    password = EXCLUDED.password,
    database = EXCLUDED.database,
    format = EXCLUDED.format,
    encoding = EXCLUDED.encoding;

-- Admin user
-- Password: admin (hashed with Argon2)
INSERT INTO users (
    id, login, password, firstname, lastname, 
    account_type, fee, public_type, status, 
    crea_date, modif_date, language
) VALUES (
    1, 
    'admin', 
    '$argon2id$v=19$m=102400,t=2,p=8$XDb+UZsMVwXlf+7UXaNuag$Bx9DmG8e8GbueweE/PNpsQ',
    'Admin', 
    'System',
    'admin',
    'free',
    97,
    0,
    NOW(),
    NOW(),
    'fr'
)
ON CONFLICT (id) DO UPDATE SET
    login = EXCLUDED.login,
    password = EXCLUDED.password,
    firstname = EXCLUDED.firstname,
    lastname = EXCLUDED.lastname,
    account_type = EXCLUDED.account_type,
    fee = EXCLUDED.fee,
    modif_date = NOW();

-- =============================================================================
-- RESET SEQUENCES
-- =============================================================================

-- Sequences removed for account_types and fees (using code as primary key)
SELECT setval('z3950servers_id_seq', COALESCE((SELECT MAX(id) FROM z3950servers), 1), true);
SELECT setval('users_id_seq', COALESCE((SELECT MAX(id) FROM users), 1), true);
SELECT setval('authors_id_seq', COALESCE((SELECT MAX(id) FROM authors), 1), true);
SELECT setval('editions_id_seq', COALESCE((SELECT MAX(id) FROM editions), 1), true);
SELECT setval('collections_id_seq', COALESCE((SELECT MAX(id) FROM collections), 1), true);
SELECT setval('series_id_seq', COALESCE((SELECT MAX(id) FROM series), 1), true);
SELECT setval('sources_id_seq', COALESCE((SELECT MAX(id) FROM sources), 1), true);
SELECT setval('items_id_seq', COALESCE((SELECT MAX(id) FROM items), 1), true);
SELECT setval('remote_items_id_seq', COALESCE((SELECT MAX(id) FROM remote_items), 1), true);
SELECT setval('specimens_id_seq', COALESCE((SELECT MAX(id) FROM specimens), 1), true);
SELECT setval('remote_specimens_id_seq', COALESCE((SELECT MAX(id) FROM remote_specimens), 1), true);
SELECT setval('loans_id_seq', COALESCE((SELECT MAX(id) FROM loans), 1), true);
SELECT setval('loans_archives_id_seq', COALESCE((SELECT MAX(id) FROM loans_archives), 1), true);
SELECT setval('loans_settings_id_seq', COALESCE((SELECT MAX(id) FROM loans_settings), 1), true);

-- =============================================================================
-- VERIFICATION
-- =============================================================================

DO $$
DECLARE
    account_types_count INTEGER;
    fees_count INTEGER;
    z3950_count INTEGER;
    admin_exists BOOLEAN;
    tables_count INTEGER;
BEGIN
    SELECT COUNT(*) INTO account_types_count FROM account_types;
    SELECT COUNT(*) INTO fees_count FROM fees;
    SELECT COUNT(*) INTO z3950_count FROM z3950servers;
    SELECT EXISTS(SELECT 1 FROM users WHERE login = 'admin') INTO admin_exists;
    SELECT COUNT(*) INTO tables_count FROM information_schema.tables 
        WHERE table_schema = 'public' 
        AND table_type = 'BASE TABLE'
        AND table_name IN (
            'account_types', 'fees', 'users', 'authors', 'editions',
            'collections', 'series', 'sources', 'items', 'remote_items', 'specimens',
            'remote_specimens', 'loans', 'loans_archives', 'loans_settings', 'z3950servers'
        );
    
    RAISE NOTICE '========================================';
    RAISE NOTICE 'Database initialization completed!';
    RAISE NOTICE '========================================';
    RAISE NOTICE 'Tables created: %', tables_count;
    RAISE NOTICE '  - Account types: %', account_types_count;
    RAISE NOTICE '  - Fees: %', fees_count;
    RAISE NOTICE '  - Z39.50 servers: %', z3950_count;
    RAISE NOTICE '  - Admin user exists: %', admin_exists;
    RAISE NOTICE '';
    RAISE NOTICE 'Default admin credentials:';
    RAISE NOTICE '  Login: admin';
    RAISE NOTICE '  Password: admin';
    RAISE NOTICE '';
    RAISE NOTICE '⚠️  IMPORTANT: Change the admin password after first login!';
    RAISE NOTICE '========================================';
END $$;
