-- =============================================================================
-- Elidune Legacy Database Sample Data
-- =============================================================================
-- This file simulates a database in the old C version format
-- to test the migration to the new Rust version.
--
-- Usage:
--   psql -U postgres -c "CREATE DATABASE elidune_legacy OWNER elidune;"
--   psql -U elidune -d elidune_legacy -f scripts/sample_legacy_data.sql
--
-- Then run the migration:
--   python scripts/migrate_data.py \
--     --source-db "postgres://elidune:elidune@localhost/elidune_legacy" \
--     --target-db "postgres://elidune:elidune@localhost/elidune"
-- =============================================================================

-- Cleanup if tables exist
DROP TABLE IF EXISTS borrows_archives CASCADE;
DROP TABLE IF EXISTS borrows CASCADE;
DROP TABLE IF EXISTS borrows_settings CASCADE;
DROP TABLE IF EXISTS remote_specimens CASCADE;
DROP TABLE IF EXISTS specimens CASCADE;
DROP TABLE IF EXISTS remote_items CASCADE;
DROP TABLE IF EXISTS items CASCADE;
DROP TABLE IF EXISTS z3950servers CASCADE;
DROP TABLE IF EXISTS fees CASCADE;
DROP TABLE IF EXISTS users CASCADE;
DROP TABLE IF EXISTS account_types CASCADE;
DROP TABLE IF EXISTS authors CASCADE;
DROP TABLE IF EXISTS editions CASCADE;
DROP TABLE IF EXISTS collections CASCADE;
DROP TABLE IF EXISTS series CASCADE;
DROP TABLE IF EXISTS sources CASCADE;

-- =============================================================================
-- REFERENCE TABLES
-- =============================================================================

CREATE TABLE account_types (
    id SERIAL PRIMARY KEY,
    name VARCHAR,
    items_rights CHAR(1) DEFAULT 'n',
    users_rights CHAR(1) DEFAULT 'n',
    loans_rights CHAR(1) DEFAULT 'n',
    items_archive_rights CHAR(1) DEFAULT 'n',
    borrows_rights CHAR(1),
    settings_rights CHAR(1)
);

INSERT INTO account_types (id, name, items_rights, users_rights, loans_rights, items_archive_rights, borrows_rights, settings_rights) VALUES
(1, 'Guest', 'r', 'r', 'n', 'n', 'n', 'r'),
(2, 'Reader', 'r', 'r', 'r', 'r', 'r', 'r'),
(3, 'Librarian', 'w', 'w', 'w', 'w', 'w', 'r'),
(4, 'Administrator', 'w', 'w', 'w', 'w', 'w', 'w'),
(8, 'Group', 'r', 'r', 'r', 'r', 'r', 'r');

SELECT setval('account_types_id_seq', 10);

-- =============================================================================
-- USERS
-- =============================================================================

CREATE TABLE users (
    id SERIAL PRIMARY KEY,
    login VARCHAR,
    password VARCHAR,  -- Old plaintext passwords (legacy!)
    firstname VARCHAR,
    lastname VARCHAR,
    email VARCHAR,
    addr_street VARCHAR,
    addr_zip_code INTEGER,
    addr_city VARCHAR,
    phone VARCHAR,
    sex_id SMALLINT,
    account_type_id SMALLINT,
    subscription_type_id SMALLINT,
    fee_id SMALLINT,
    last_payement_date TIMESTAMP DEFAULT NOW(),
    group_id INTEGER,
    barcode VARCHAR,
    notes VARCHAR,
    occupation VARCHAR,
    crea_date INTEGER,
    modif_date INTEGER,
    issue_date INTEGER,
    birthdate VARCHAR,
    archived_date INTEGER DEFAULT 0,
    public_type INTEGER
);

-- Test users (plaintext passwords as in the old version)
INSERT INTO users (id, login, password, firstname, lastname, email, addr_street, addr_zip_code, addr_city, phone, sex_id, account_type_id, barcode, occupation, crea_date, modif_date, public_type) VALUES
(1, 'admin', 'admin', 'Admin', 'Système', 'admin@bibliotheque.fr', NULL, NULL, NULL, NULL, 1, 4, NULL, NULL, 1704067200, 1704067200, 97),
(2, 'biblio', 'biblio123', 'Marie', 'Dupont', 'marie.dupont@bibliotheque.fr', '12 rue des Livres', 75001, 'Paris', '0612345678', 2, 3, 'BIB001', 'Bibliothécaire', 1704067200, 1704067200, 97),
(3, 'lecteur1', 'pass123', 'Jean', 'Martin', 'jean.martin@email.fr', '45 avenue Victor Hugo', 69001, 'Lyon', '0698765432', 1, 2, 'LECT001', 'Enseignant', 1704153600, 1704153600, 97),
(4, 'lecteur2', 'pass456', 'Sophie', 'Bernard', 'sophie.bernard@email.fr', '8 place Bellecour', 69002, 'Lyon', '0611223344', 2, 2, 'LECT002', 'Étudiant', 1704240000, 1704240000, 106),
(5, 'lecteur3', 'pass789', 'Pierre', 'Petit', 'pierre.petit@email.fr', '23 rue Pasteur', 33000, 'Bordeaux', '0655443322', 1, 2, 'LECT003', 'Retraité', 1704326400, 1704326400, 97),
(6, 'enfant1', 'enfant', 'Lucas', 'Moreau', NULL, '15 rue des Écoles', 75005, 'Paris', NULL, 1, 2, 'ENF001', 'Écolier', 1704412800, 1704412800, 106),
(7, 'invite', 'invite', 'Visiteur', 'Anonyme', NULL, NULL, NULL, NULL, NULL, 0, 1, NULL, NULL, 1704499200, 1704499200, 117);

SELECT setval('users_id_seq', 10);

-- =============================================================================
-- AUTHORS
-- =============================================================================

CREATE TABLE authors (
    id SERIAL PRIMARY KEY,
    key VARCHAR UNIQUE,
    lastname VARCHAR,
    firstname VARCHAR,
    bio VARCHAR,
    notes VARCHAR
);

INSERT INTO authors (id, key, lastname, firstname, bio, notes) VALUES
(1, 'hugo_victor', 'Hugo', 'Victor', 'Écrivain français du XIXe siècle, figure majeure du romantisme.', NULL),
(2, 'tolkien_jrr', 'Tolkien', 'J.R.R.', 'Écrivain britannique, auteur du Seigneur des Anneaux.', NULL),
(3, 'rowling_jk', 'Rowling', 'J.K.', 'Auteure britannique de la série Harry Potter.', NULL),
(4, 'goscinny_rene', 'Goscinny', 'René', 'Scénariste de bande dessinée français.', NULL),
(5, 'uderzo_albert', 'Uderzo', 'Albert', 'Dessinateur de bande dessinée français.', NULL),
(6, 'herge', 'Hergé', NULL, 'Auteur belge de bande dessinée, créateur de Tintin.', 'Pseudonyme de Georges Remi'),
(7, 'verne_jules', 'Verne', 'Jules', 'Écrivain français, pionnier de la science-fiction.', NULL),
(8, 'saint_exupery', 'Saint-Exupéry', 'Antoine de', 'Écrivain et aviateur français.', NULL),
(9, 'dumas_alexandre', 'Dumas', 'Alexandre', 'Écrivain français, auteur des Trois Mousquetaires.', NULL),
(10, 'moliere', 'Molière', NULL, 'Dramaturge et comédien français du XVIIe siècle.', 'Pseudonyme de Jean-Baptiste Poquelin');

SELECT setval('authors_id_seq', 20);

-- =============================================================================
-- PUBLISHERS
-- =============================================================================

CREATE TABLE editions (
    id SERIAL PRIMARY KEY,
    key VARCHAR,
    name VARCHAR,
    place VARCHAR,
    notes VARCHAR
);

INSERT INTO editions (id, key, name, place, notes) VALUES
(1, 'gallimard', 'Gallimard', 'Paris', NULL),
(2, 'folio', 'Folio', 'Paris', 'Gallimard paperback collection'),
(3, 'pocket', 'Pocket', 'Paris', NULL),
(4, 'livre_poche', 'Le Livre de Poche', 'Paris', NULL),
(5, 'dargaud', 'Dargaud', 'Paris', 'Comic book publisher'),
(6, 'casterman', 'Casterman', 'Bruxelles', 'Belgian publisher'),
(7, 'hachette', 'Hachette', 'Paris', NULL),
(8, 'flammarion', 'Flammarion', 'Paris', NULL);

SELECT setval('editions_id_seq', 20);

-- =============================================================================
-- COLLECTIONS
-- =============================================================================

CREATE TABLE collections (
    id SERIAL PRIMARY KEY,
    key VARCHAR,
    title1 VARCHAR,
    title2 VARCHAR,
    title3 VARCHAR,
    issn VARCHAR
);

INSERT INTO collections (id, key, title1, title2, title3, issn) VALUES
(1, 'folio_classique', 'Folio Classique', NULL, NULL, NULL),
(2, 'pleiade', 'Bibliothèque de la Pléiade', NULL, NULL, NULL),
(3, 'harry_potter', 'Harry Potter', NULL, NULL, NULL),
(4, 'asterix', 'Astérix', NULL, NULL, NULL),
(5, 'tintin', 'Les Aventures de Tintin', NULL, NULL, NULL);

SELECT setval('collections_id_seq', 10);

-- =============================================================================
-- SERIES
-- =============================================================================

CREATE TABLE series (
    id SERIAL PRIMARY KEY,
    key VARCHAR,
    name VARCHAR
);

INSERT INTO series (id, key, name) VALUES
(1, 'sda', 'Le Seigneur des Anneaux'),
(2, 'hp', 'Harry Potter'),
(3, 'asterix', 'Astérix'),
(4, 'tintin', 'Tintin'),
(5, 'mousquetaires', 'Les Mousquetaires');

SELECT setval('series_id_seq', 10);

-- =============================================================================
-- SOURCES (acquisition origins)
-- =============================================================================

CREATE TABLE sources (
    id SERIAL PRIMARY KEY,
    key VARCHAR,
    name VARCHAR
);

INSERT INTO sources (id, key, name) VALUES
(1, 'achat', 'Purchase'),
(2, 'don', 'Donation'),
(3, 'depot', 'Legal deposit'),
(4, 'echange', 'Exchange');

SELECT setval('sources_id_seq', 10);

-- =============================================================================
-- ITEMS (bibliographic records)
-- =============================================================================

CREATE TABLE items (
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
    nb_specimens SMALLINT DEFAULT 0,
    state VARCHAR,
    is_archive SMALLINT DEFAULT 0,
    archived_timestamp INTEGER,
    is_valid SMALLINT DEFAULT 1,
    crea_date INTEGER,
    modif_date INTEGER
);

-- Books
INSERT INTO items (id, media_type, identification, title1, title2, author1_ids, author1_functions, edition_id, edition_date, publication_date, nb_pages, lang, public_type, genre, subject, keywords, serie_id, serie_vol_number, collection_id, source_id, nb_specimens, crea_date, modif_date) VALUES
(1, 'b', '978-2-07-040850-4', 'Les Misérables', 'Tome 1 - Fantine', ARRAY[1], '70', 2, '2018', '1862', '512 p.', 1, 97, 1, 'Roman historique', 'hugo, misérables, france, 19e siècle', NULL, NULL, 1, 1, 2, 1704067200, 1704067200),
(2, 'b', '978-2-07-061202-4', 'Le Seigneur des Anneaux', 'La Communauté de l''Anneau', ARRAY[2], '70', 1, '2022', '1954', '576 p.', 1, 97, 2, 'Fantasy', 'tolkien, fantasy, anneaux, hobbits', 1, 1, NULL, 1, 3, 1704067200, 1704067200),
(3, 'b', '978-2-07-054127-0', 'Harry Potter à l''école des sorciers', NULL, ARRAY[3], '70', 1, '2017', '1997', '320 p.', 1, 106, 2, 'Fantasy jeunesse', 'harry potter, magie, sorciers, poudlard', 2, 1, 3, 1, 2, 1704153600, 1704153600),
(4, 'b', '978-2-07-036024-6', 'Le Petit Prince', NULL, ARRAY[8], '70', 1, '2020', '1943', '96 p.', 1, 106, 3, 'Conte philosophique', 'petit prince, conte, philosophie, enfants', NULL, NULL, NULL, 2, 1, 1704153600, 1704153600),
(5, 'b', '978-2-07-040572-5', 'Vingt mille lieues sous les mers', NULL, ARRAY[7], '70', 4, '2019', '1870', '480 p.', 1, 97, 4, 'Science-fiction', 'verne, sous-marin, nautilus, aventure', NULL, NULL, NULL, 1, 1, 1704240000, 1704240000),
(6, 'b', '978-2-07-041239-6', 'Les Trois Mousquetaires', NULL, ARRAY[9], '70', 2, '2021', '1844', '864 p.', 1, 97, 1, 'Roman de cape et d''épée', 'dumas, mousquetaires, aventure, france', 5, 1, 1, 1, 2, 1704240000, 1704240000),
(7, 'b', '978-2-07-041044-6', 'L''Avare', 'Comédie en cinq actes', ARRAY[10], '70', 2, '2018', '1668', '128 p.', 1, 97, 5, 'Théâtre classique', 'molière, théâtre, comédie, avarice', NULL, NULL, 1, 1, 1, 1704326400, 1704326400);

-- Comics
INSERT INTO items (id, media_type, identification, title1, title2, author1_ids, author1_functions, author2_ids, author2_functions, edition_id, edition_date, publication_date, nb_pages, lang, public_type, genre, subject, keywords, serie_id, serie_vol_number, collection_id, source_id, nb_specimens, crea_date, modif_date) VALUES
(8, 'bc', '978-2-01-210034-8', 'Astérix le Gaulois', NULL, ARRAY[4], '690', ARRAY[5], '440', 5, '2019', '1961', '48 p.', 1, 106, 6, 'Bande dessinée', 'astérix, gaulois, romains, bd', 3, 1, 4, 1, 2, 1704326400, 1704326400),
(9, 'bc', '978-2-01-210035-5', 'Astérix et Cléopâtre', NULL, ARRAY[4], '690', ARRAY[5], '440', 5, '2019', '1965', '48 p.', 1, 106, 6, 'Bande dessinée', 'astérix, égypte, cléopâtre, bd', 3, 6, 4, 1, 1, 1704412800, 1704412800),
(10, 'bc', '978-2-203-00101-9', 'Tintin au Tibet', NULL, ARRAY[6], '70', NULL, NULL, 6, '2018', '1960', '62 p.', 1, 106, 6, 'Bande dessinée', 'tintin, tibet, aventure, bd', 4, 20, 5, 2, 1, 1704412800, 1704412800);

-- Audio
INSERT INTO items (id, media_type, identification, title1, title2, author1_ids, author1_functions, edition_id, edition_date, publication_date, lang, public_type, genre, subject, keywords, source_id, nb_specimens, crea_date, modif_date) VALUES
(11, 'amc', '0602557382594', 'Abbey Road', 'The Beatles', NULL, NULL, NULL, '2019', '1969', 2, 97, 10, 'Rock', 'beatles, rock, classique, abbey road', 1, 1, 1704499200, 1704499200),
(12, 'amc', '0602547288271', 'Random Access Memories', 'Daft Punk', NULL, NULL, NULL, '2013', '2013', 2, 97, 11, 'Electronic', 'daft punk, electro, dance', 1, 1, 1704499200, 1704499200);

-- DVD
INSERT INTO items (id, media_type, identification, title1, title2, author1_ids, author1_functions, edition_id, edition_date, publication_date, lang, public_type, genre, subject, keywords, source_id, nb_specimens, crea_date, modif_date) VALUES
(13, 'vd', '3475001058423', 'Le Seigneur des Anneaux', 'La Communauté de l''Anneau - Version longue', NULL, NULL, NULL, '2021', '2001', 1, 97, 2, 'Fantasy', 'tolkien, anneaux, jackson, fantasy', 1, 1, 1704585600, 1704585600),
(14, 'vd', '3333973198380', 'Astérix et Obélix : Mission Cléopâtre', NULL, NULL, NULL, NULL, '2015', '2002', 1, 106, 7, 'Comedy', 'astérix, cléopâtre, comédie, chabat', 1, 1, 1704585600, 1704585600);

SELECT setval('items_id_seq', 50);

-- =============================================================================
-- SPECIMENS (physical copies)
-- =============================================================================

CREATE TABLE specimens (
    id SERIAL PRIMARY KEY,
    id_item INTEGER,
    source_id INTEGER,
    identification VARCHAR,
    cote VARCHAR,
    place SMALLINT,
    status SMALLINT DEFAULT 98,  -- 98 = borrowable
    codestat SMALLINT,
    notes VARCHAR,
    price VARCHAR,
    modif_date INTEGER,
    is_archive INTEGER DEFAULT 0,
    archive_date INTEGER DEFAULT 0,
    crea_date INTEGER
);

-- Book specimens
INSERT INTO specimens (id, id_item, source_id, identification, cote, place, status, price, crea_date, modif_date) VALUES
(1, 1, 1, 'LIV-001-A', 'R HUG m1', 1, 98, '8.90', 1704067200, 1704067200),
(2, 1, 2, 'LIV-001-B', 'R HUG m1', 1, 98, NULL, 1704067200, 1704067200),
(3, 2, 1, 'LIV-002-A', 'R TOL s1', 1, 98, '12.50', 1704067200, 1704067200),
(4, 2, 1, 'LIV-002-B', 'R TOL s1', 1, 98, '12.50', 1704067200, 1704067200),
(5, 2, 2, 'LIV-002-C', 'R TOL s1', 2, 98, NULL, 1704067200, 1704067200),
(6, 3, 1, 'LIV-003-A', 'J ROW h1', 1, 98, '7.90', 1704153600, 1704153600),
(7, 3, 1, 'LIV-003-B', 'J ROW h1', 1, 98, '7.90', 1704153600, 1704153600),
(8, 4, 2, 'LIV-004-A', 'J SAI p', 1, 98, NULL, 1704153600, 1704153600),
(9, 5, 1, 'LIV-005-A', 'R VER v', 1, 98, '9.50', 1704240000, 1704240000),
(10, 6, 1, 'LIV-006-A', 'R DUM t1', 1, 98, '11.00', 1704240000, 1704240000),
(11, 6, 1, 'LIV-006-B', 'R DUM t1', 2, 110, '11.00', 1704240000, 1704240000),  -- 110 = not borrowable
(12, 7, 1, 'LIV-007-A', 'T MOL a', 1, 98, '5.50', 1704326400, 1704326400);

-- Comic specimens
INSERT INTO specimens (id, id_item, source_id, identification, cote, place, status, price, crea_date, modif_date) VALUES
(13, 8, 1, 'BD-001-A', 'BD AST 1', 1, 98, '10.95', 1704326400, 1704326400),
(14, 8, 1, 'BD-001-B', 'BD AST 1', 1, 98, '10.95', 1704326400, 1704326400),
(15, 9, 1, 'BD-002-A', 'BD AST 6', 1, 98, '10.95', 1704412800, 1704412800),
(16, 10, 2, 'BD-003-A', 'BD TIN 20', 1, 98, NULL, 1704412800, 1704412800);

-- Audio/video specimens
INSERT INTO specimens (id, id_item, source_id, identification, cote, place, status, price, crea_date, modif_date) VALUES
(17, 11, 1, 'CD-001-A', 'CD BEA a', 1, 98, '15.99', 1704499200, 1704499200),
(18, 12, 1, 'CD-002-A', 'CD DAF r', 1, 98, '18.99', 1704499200, 1704499200),
(19, 13, 1, 'DVD-001-A', 'DVD SDA 1', 1, 98, '24.99', 1704585600, 1704585600),
(20, 14, 1, 'DVD-002-A', 'DVD AST m', 1, 98, '14.99', 1704585600, 1704585600);

SELECT setval('specimens_id_seq', 50);

-- =============================================================================
-- LOANS (borrows)
-- =============================================================================

CREATE TABLE borrows (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL,
    specimen_id INTEGER NOT NULL,
    item_id INTEGER,
    date INTEGER NOT NULL,
    renew_date INTEGER,
    nb_renews SMALLINT DEFAULT 0,
    issue_date INTEGER,
    notes VARCHAR,
    returned_date INTEGER
);

-- Some current loans
INSERT INTO borrows (id, user_id, specimen_id, item_id, date, issue_date, nb_renews, notes) VALUES
(1, 3, 3, 2, 1735689600, 1737504000, 0, NULL),           -- Jean Martin borrowed Lord of the Rings
(2, 3, 6, 3, 1735776000, 1737590400, 0, NULL),           -- Jean Martin borrowed Harry Potter
(3, 4, 13, 8, 1735862400, 1737676800, 1, 'Renewed'),     -- Sophie Bernard borrowed Asterix
(4, 5, 17, 11, 1735948800, 1737763200, 0, NULL),         -- Pierre Petit borrowed Abbey Road
(5, 6, 16, 10, 1736035200, 1737849600, 0, NULL);         -- Lucas Moreau borrowed Tintin

-- Some returned loans
INSERT INTO borrows (id, user_id, specimen_id, item_id, date, issue_date, nb_renews, returned_date, notes) VALUES
(6, 3, 1, 1, 1733011200, 1734825600, 0, 1734566400, NULL),        -- Les Misérables returned
(7, 4, 8, 4, 1733097600, 1734912000, 0, 1734652800, NULL),        -- Le Petit Prince returned
(8, 5, 9, 5, 1733184000, 1734998400, 1, 1735084800, 'Good condition');  -- Twenty Thousand Leagues returned

SELECT setval('borrows_id_seq', 20);

-- =============================================================================
-- LOAN ARCHIVES
-- =============================================================================

CREATE TABLE borrows_archives (
    id SERIAL PRIMARY KEY,
    item_id INTEGER NOT NULL,
    specimen_id INTEGER,
    date INTEGER NOT NULL,
    nb_renews SMALLINT,
    issue_date INTEGER,
    returned_date INTEGER,
    notes VARCHAR,
    borrower_public_type INTEGER,
    occupation VARCHAR,
    addr_city VARCHAR,
    sex_id SMALLINT,
    account_type_id SMALLINT
);

-- Archived loans history (previous year)
INSERT INTO borrows_archives (id, item_id, specimen_id, date, nb_renews, issue_date, returned_date, borrower_public_type, occupation, addr_city, sex_id, account_type_id) VALUES
(1, 1, 1, 1701388800, 0, 1703203200, 1702598400, 97, 'Teacher', 'Lyon', 1, 2),
(2, 2, 3, 1701475200, 1, 1703289600, 1703116800, 97, 'Teacher', 'Lyon', 1, 2),
(3, 3, 6, 1701561600, 0, 1703376000, 1702857600, 106, 'Student', 'Lyon', 2, 2),
(4, 8, 13, 1701648000, 0, 1703462400, 1702944000, 106, 'Schoolchild', 'Paris', 1, 2),
(5, 4, 8, 1701734400, 0, 1703548800, 1703030400, 106, 'Schoolchild', 'Paris', 1, 2);

SELECT setval('borrows_archives_id_seq', 10);

-- =============================================================================
-- LOAN SETTINGS
-- =============================================================================

CREATE TABLE borrows_settings (
    id SERIAL PRIMARY KEY,
    media_type VARCHAR,
    nb_max SMALLINT,
    nb_renews SMALLINT,
    duration SMALLINT,
    notes VARCHAR,
    account_type_id SMALLINT
);

INSERT INTO borrows_settings (id, media_type, nb_max, nb_renews, duration, notes) VALUES
(1, 'b', 5, 2, 21, 'Books'),
(2, 'bc', 5, 1, 14, 'Comics'),
(3, 'p', 3, 0, 7, 'Periodicals'),
(4, 'amc', 3, 1, 14, 'Audio CDs'),
(5, 'vd', 2, 1, 7, 'DVDs');

SELECT setval('borrows_settings_id_seq', 10);

-- =============================================================================
-- Z39.50 SERVERS
-- =============================================================================

CREATE TABLE z3950servers (
    id SERIAL PRIMARY KEY,
    address VARCHAR,
    port INTEGER DEFAULT 2200,
    name VARCHAR,
    description VARCHAR,
    activated INTEGER DEFAULT 0,
    login VARCHAR,
    password VARCHAR,
    database VARCHAR,
    format VARCHAR
);

INSERT INTO z3950servers (id, name, address, port, database, format, activated, description) VALUES
(1, 'BnF - General Catalog', 'z3950.bnf.fr', 2211, 'TOUT-UTF8', 'UNIMARC', 1, 'French National Library'),
(2, 'SUDOC', 'z3950.sudoc.fr', 2100, 'default', 'UNIMARC', 1, 'French University Documentation System'),
(3, 'Library of Congress', 'z3950.loc.gov', 7090, 'VOYAGER', 'MARC21', 0, 'US Library of Congress');

SELECT setval('z3950servers_id_seq', 10);

-- =============================================================================
-- FEES
-- =============================================================================

CREATE TABLE fees (
    id SERIAL PRIMARY KEY,
    "desc" VARCHAR,
    amount INTEGER DEFAULT 0
);

INSERT INTO fees (id, "desc", amount) VALUES
(1, 'Annual adult subscription', 1500),
(2, 'Annual youth subscription', 800),
(3, 'Annual family subscription', 2500),
(4, 'Temporary card (3 months)', 500);

SELECT setval('fees_id_seq', 10);

-- =============================================================================
-- REMOTE ITEMS (Z39.50 cache)
-- =============================================================================

CREATE TABLE remote_items (
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
    archived_timestamp INTEGER,
    is_valid SMALLINT DEFAULT 0,
    modif_date INTEGER,
    crea_date INTEGER
);

CREATE TABLE remote_specimens (
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
    creation_date INTEGER,
    modif_date INTEGER
);

-- =============================================================================
-- INDEXES
-- =============================================================================

CREATE INDEX users_id_key ON users (id);
CREATE INDEX users_login_key ON users (login);
CREATE INDEX authors_id_key ON authors (id);
CREATE INDEX authors_lastname_key ON authors (lastname);
CREATE INDEX editions_id_key ON editions (id);
CREATE INDEX items_id_key ON items (id);
CREATE INDEX items_identification_key ON items (identification);
CREATE INDEX items_title1_key ON items (title1);
CREATE INDEX specimens_id_key ON specimens (id);
CREATE INDEX specimens_id_item_key ON specimens (id_item);
CREATE INDEX specimens_identification_key ON specimens (identification);
CREATE INDEX borrows_id_key ON borrows (id);
CREATE INDEX borrows_user_id_key ON borrows (user_id);
CREATE INDEX borrows_specimen_id_key ON borrows (specimen_id);

-- =============================================================================
-- END
-- =============================================================================

-- Display summary
DO $$
BEGIN
    RAISE NOTICE '';
    RAISE NOTICE '=== Elidune legacy database created successfully ===';
    RAISE NOTICE '';
    RAISE NOTICE 'Statistics:';
    RAISE NOTICE '  - Users: 7 (admin, librarian, 4 readers, 1 guest)';
    RAISE NOTICE '  - Authors: 10';
    RAISE NOTICE '  - Items: 14 (7 books, 3 comics, 2 CDs, 2 DVDs)';
    RAISE NOTICE '  - Specimens: 20';
    RAISE NOTICE '  - Current loans: 5';
    RAISE NOTICE '  - Archived loans: 5';
    RAISE NOTICE '  - Z39.50 servers: 3 (2 active)';
    RAISE NOTICE '';
    RAISE NOTICE 'Test accounts:';
    RAISE NOTICE '  - admin / admin (Administrator)';
    RAISE NOTICE '  - biblio / biblio123 (Librarian)';
    RAISE NOTICE '  - lecteur1 / pass123 (Reader)';
    RAISE NOTICE '';
END $$;

