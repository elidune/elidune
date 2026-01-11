#!/usr/bin/env python3
"""
Data migration script for Elidune
Migrates data from the old C/XML-RPC PostgreSQL schema to the new Rust schema.

Usage:
    python migrate_data.py --source-db <old_db_url> --target-db <new_db_url>

Options:
    --reset         Reset (truncate) all tables in target database before migration
    --skip-items    Skip items migration
    --skip-users    Skip users migration
    -y, --yes       Skip confirmation prompts

Note: 
    - Passwords are automatically hashed with Argon2 during migration.
    - The script auto-detects if date columns are INTEGER or TIMESTAMPTZ and adapts:
      * If INTEGER: keeps Unix timestamps as-is
      * If TIMESTAMPTZ: converts Unix timestamps to proper timestamps
    - This allows running the script before or after the date conversion migration.
    - Returned loans (borrows with returned_date) are migrated to loans_archives
    - User occupations are mapped to occupation_id codes
    - All schema migrations are integrated in this script (no external SQL files needed):
      * Initial schema creation
      * Password hash management
      * Full-text search setup
      * Date conversion to TIMESTAMPTZ
      * Renamed borrows* tables to loans*
      * Removed sex_id from users and loans_archives
      * Added status, occupation_id, language to users
      * Added lifecycle_status and archived_date to items/specimens
      * Login is required and unique, email is optional

Requirements:
    pip install psycopg2-binary argon2-cffi
"""

import argparse
import psycopg2
from psycopg2 import IntegrityError
import sys
import os
import subprocess
from pathlib import Path

# Try to import argon2 for password hashing
try:
    from argon2 import PasswordHasher
    ARGON2_AVAILABLE = True
except ImportError:
    ARGON2_AVAILABLE = False
    print("Warning: argon2-cffi not installed. Password hashing will be disabled.")
    print("Install with: pip install argon2-cffi")


def connect_db(url):
    """Connect to PostgreSQL database."""
    return psycopg2.connect(url)


def hash_password(password, hasher):
    """Hash a password using Argon2."""
    if password is None or password == '':
        return None
    return hasher.hash(password)


def detect_date_column_type(conn, table, column):
    """Detect if a date column is INTEGER or TIMESTAMPTZ in the target database."""
    cur = conn.cursor()
    cur.execute("""
        SELECT data_type 
        FROM information_schema.columns 
        WHERE table_name = %s AND column_name = %s
    """, (table, column))
    result = cur.fetchone()
    if result:
        return result[0]
    return None


def is_timestamptz_schema(conn):
    """Check if the target database uses TIMESTAMPTZ for date columns."""
    col_type = detect_date_column_type(conn, 'users', 'crea_date')
    return col_type is not None and 'timestamp' in col_type.lower()


def convert_date(value, use_timestamptz):
    """Convert a Unix timestamp to TIMESTAMPTZ if needed, otherwise keep as INTEGER."""
    if not use_timestamptz:
        return value
    
    # Convert INTEGER to TIMESTAMPTZ
    if value is None or value == 0:
        return None
    try:
        from datetime import datetime, timezone
        return datetime.fromtimestamp(value, tz=timezone.utc)
    except (OSError, ValueError, OverflowError, TypeError):
        return None


def run_schema_migrations(conn):
    """Run all schema migrations in order."""
    print("Running schema migrations...")
    
    cur = conn.cursor()
    
    try:
        # Migration 1: Initial schema
        print("  Running initial_schema...")
        run_migration_initial_schema(cur)
        conn.commit()
        print("    ✓ initial_schema completed")
        
        # Migration 2: Add password hash (remove password column, add indexes)
        print("  Running add_password_hash...")
        run_migration_add_password_hash(cur)
        conn.commit()
        print("    ✓ add_password_hash completed")
        
        # Migration 3: Add fulltext search
        print("  Running add_fulltext_search...")
        run_migration_add_fulltext_search(cur)
        conn.commit()
        print("    ✓ add_fulltext_search completed")
        
        # Migration 4: Convert dates to TIMESTAMPTZ
        print("  Running convert_dates_to_timestamptz...")
        run_migration_convert_dates_to_timestamptz(cur)
        conn.commit()
        print("    ✓ convert_dates_to_timestamptz completed")
        
        # Migration 5: Rename borrows and add user status
        print("  Running rename_borrows_and_user_status...")
        run_migration_rename_borrows_and_user_status(cur)
        conn.commit()
        print("    ✓ rename_borrows_and_user_status completed")
        
        # Migration 6: Add status to items/specimens
        print("  Running add_status_to_items_specimens...")
        run_migration_add_status_to_items_specimens(cur)
        conn.commit()
        print("    ✓ add_status_to_items_specimens completed")
        
        # Migration 7: Add language to users
        print("  Running add_language_to_users...")
        run_migration_add_language_to_users(cur)
        conn.commit()
        print("    ✓ add_language_to_users completed")
        
        # Migration 8: Email/login constraints (partial - will be superseded)
        print("  Running email_login_constraints...")
        run_migration_email_login_constraints(cur)
        conn.commit()
        print("    ✓ email_login_constraints completed")
        
        # Migration 9: Remove sex_id
        print("  Running remove_sex_id_from_users...")
        run_migration_remove_sex_id_from_users(cur)
        conn.commit()
        print("    ✓ remove_sex_id_from_users completed")
        
        # Migration 10: Revert email constraints and use login
        print("  Running revert_email_constraints_use_login...")
        run_migration_revert_email_constraints_use_login(cur)
        conn.commit()
        print("    ✓ revert_email_constraints_use_login completed")
        
        print("  All migrations completed successfully")
        return True
        
    except Exception as e:
        conn.rollback()
        print(f"  ✗ Error running migrations: {e}")
        import traceback
        traceback.print_exc()
        return False


def run_migration_initial_schema(cur):
    """Migration 1: Initial schema creation."""
    # Account types table
    cur.execute("""
        CREATE TABLE IF NOT EXISTS account_types (
            id SERIAL PRIMARY KEY,
            name VARCHAR,
            items_rights CHAR(1) DEFAULT 'n',
            users_rights CHAR(1) DEFAULT 'n',
            loans_rights CHAR(1) DEFAULT 'n',
            items_archive_rights CHAR(1) DEFAULT 'n',
            borrows_rights CHAR(1),
            settings_rights CHAR(1)
        )
    """)
    
    # Users table
    cur.execute("""
        CREATE TABLE IF NOT EXISTS users (
            id SERIAL PRIMARY KEY,
            login VARCHAR,
            password VARCHAR,
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
        )
    """)
    cur.execute("CREATE INDEX IF NOT EXISTS users_id_key ON users (id)")
    
    # Authors table
    cur.execute("""
        CREATE TABLE IF NOT EXISTS authors (
            id SERIAL PRIMARY KEY,
            key VARCHAR UNIQUE,
            lastname VARCHAR,
            firstname VARCHAR,
            bio VARCHAR,
            notes VARCHAR
        )
    """)
    cur.execute("CREATE INDEX IF NOT EXISTS authors_id_key ON authors (id)")
    cur.execute("CREATE INDEX IF NOT EXISTS authors_lastname_key ON authors (lastname)")
    
    # Editions table
    cur.execute("""
        CREATE TABLE IF NOT EXISTS editions (
            id SERIAL PRIMARY KEY,
            key VARCHAR,
            name VARCHAR,
            place VARCHAR,
            notes VARCHAR
        )
    """)
    cur.execute("CREATE INDEX IF NOT EXISTS editions_id_key ON editions (id)")
    cur.execute("CREATE INDEX IF NOT EXISTS editions_name_key ON editions (name)")
    
    # Collections table
    cur.execute("""
        CREATE TABLE IF NOT EXISTS collections (
            id SERIAL PRIMARY KEY,
            key VARCHAR,
            title1 VARCHAR,
            title2 VARCHAR,
            title3 VARCHAR,
            issn VARCHAR
        )
    """)
    cur.execute("CREATE INDEX IF NOT EXISTS collections_id_key ON collections (id)")
    
    # Series table
    cur.execute("""
        CREATE TABLE IF NOT EXISTS series (
            id SERIAL PRIMARY KEY,
            key VARCHAR,
            name VARCHAR
        )
    """)
    cur.execute("CREATE INDEX IF NOT EXISTS series_id_key ON series (id)")
    cur.execute("CREATE INDEX IF NOT EXISTS series_name_key ON series (name)")
    
    # Sources table
    cur.execute("""
        CREATE TABLE IF NOT EXISTS sources (
            id SERIAL PRIMARY KEY,
            key VARCHAR,
            name VARCHAR
        )
    """)
    cur.execute("CREATE INDEX IF NOT EXISTS sources_id_key ON sources (id)")
    cur.execute("CREATE INDEX IF NOT EXISTS sources_name_key ON sources (name)")
    
    # Items table
    cur.execute("""
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
            crea_date INTEGER,
            modif_date INTEGER
        )
    """)
    cur.execute("CREATE INDEX IF NOT EXISTS items_id_key ON items (id)")
    cur.execute("CREATE INDEX IF NOT EXISTS items_identification_key ON items (identification)")
    cur.execute("CREATE INDEX IF NOT EXISTS items_title1_key ON items (title1)")
    
    # Remote items table
    cur.execute("""
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
            archived_timestamp INTEGER,
            is_valid SMALLINT DEFAULT 0,
            modif_date INTEGER,
            crea_date INTEGER
        )
    """)
    cur.execute("CREATE INDEX IF NOT EXISTS remote_items_id_key ON remote_items (id)")
    cur.execute("CREATE INDEX IF NOT EXISTS remote_items_identification_key ON remote_items (identification)")
    cur.execute("CREATE INDEX IF NOT EXISTS remote_items_title1_key ON remote_items (title1)")
    
    # Specimens table
    cur.execute("""
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
            modif_date INTEGER,
            is_archive INTEGER DEFAULT 0,
            archive_date INTEGER DEFAULT 0,
            crea_date INTEGER
        )
    """)
    cur.execute("CREATE INDEX IF NOT EXISTS specimens_id_key ON specimens (id)")
    cur.execute("CREATE INDEX IF NOT EXISTS specimens_id_item_key ON specimens (id_item)")
    cur.execute("CREATE INDEX IF NOT EXISTS specimens_source_id_key ON specimens (source_id)")
    cur.execute("CREATE INDEX IF NOT EXISTS specimens_identification_key ON specimens (identification)")
    
    # Remote specimens table
    cur.execute("""
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
            creation_date INTEGER,
            modif_date INTEGER
        )
    """)
    cur.execute("CREATE INDEX IF NOT EXISTS remote_specimens_id_key ON remote_specimens (id)")
    cur.execute("CREATE INDEX IF NOT EXISTS remote_specimens_id_item_key ON remote_specimens (id_item)")
    cur.execute("CREATE INDEX IF NOT EXISTS remote_specimens_identification_key ON remote_specimens (identification)")
    
    # Borrows table
    cur.execute("""
        CREATE TABLE IF NOT EXISTS borrows (
            id SERIAL PRIMARY KEY,
            user_id INTEGER NOT NULL,
            specimen_id INTEGER NOT NULL,
            item_id INTEGER,
            date INTEGER NOT NULL,
            renew_date INTEGER,
            nb_renews SMALLINT,
            issue_date INTEGER,
            notes VARCHAR,
            returned_date INTEGER
        )
    """)
    cur.execute("CREATE INDEX IF NOT EXISTS borrows_id_key ON borrows (id)")
    cur.execute("CREATE INDEX IF NOT EXISTS borrows_user_id_key ON borrows (user_id)")
    cur.execute("CREATE INDEX IF NOT EXISTS borrows_specimen_id_key ON borrows (specimen_id)")
    
    # Borrows archives table
    cur.execute("""
        CREATE TABLE IF NOT EXISTS borrows_archives (
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
        )
    """)
    cur.execute("CREATE INDEX IF NOT EXISTS borrows_archives_id_key ON borrows_archives (id)")
    cur.execute("CREATE INDEX IF NOT EXISTS borrows_archives_item_id_key ON borrows_archives (item_id)")
    
    # Borrows settings table
    cur.execute("""
        CREATE TABLE IF NOT EXISTS borrows_settings (
            id SERIAL PRIMARY KEY,
            media_type VARCHAR,
            nb_max SMALLINT,
            nb_renews SMALLINT,
            duration SMALLINT,
            notes VARCHAR,
            account_type_id SMALLINT
        )
    """)
    cur.execute("CREATE INDEX IF NOT EXISTS borrows_settings_id_key ON borrows_settings (id)")
    cur.execute("CREATE INDEX IF NOT EXISTS borrows_settings_media_type_key ON borrows_settings (media_type)")
    
    # Z39.50 servers table
    cur.execute("""
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
            format VARCHAR
        )
    """)
    
    # Fees table
    cur.execute("""
        CREATE TABLE IF NOT EXISTS fees (
            id SERIAL PRIMARY KEY,
            "desc" VARCHAR,
            amount INTEGER DEFAULT 0
        )
    """)
    
    # Insert default account types
    cur.execute("""
        INSERT INTO account_types (id, name, items_rights, users_rights, loans_rights, items_archive_rights, borrows_rights, settings_rights) VALUES
        (1, 'Guest', 'r', 'r', 'n', 'n', 'n', 'r'),
        (2, 'Reader', 'r', 'r', 'r', 'r', 'r', 'r'),
        (3, 'Librarian', 'w', 'w', 'w', 'w', 'w', 'r'),
        (4, 'Administrator', 'w', 'w', 'w', 'w', 'w', 'w'),
        (8, 'Group', 'r', 'r', 'r', 'r', 'r', 'r')
        ON CONFLICT (id) DO NOTHING
    """)
    
    # Insert default admin user
    cur.execute("""
        DO $$
        BEGIN
            IF EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'users' AND column_name = 'password') THEN
                INSERT INTO users (id, login, password, firstname, lastname, account_type_id, public_type) 
                VALUES (1, 'admin', 'admin', 'Admin', 'System', 4, 97)
                ON CONFLICT (id) DO NOTHING;
            ELSIF EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'users' AND column_name = 'password_hash') THEN
                INSERT INTO users (id, login, password_hash, firstname, lastname, account_type_id, public_type) 
                VALUES (1, 'admin', 'admin', 'Admin', 'System', 4, 97)
                ON CONFLICT (id) DO NOTHING;
            END IF;
        END $$;
    """)
    
    # Reset sequences
    cur.execute("SELECT setval('account_types_id_seq', COALESCE((SELECT MAX(id) FROM account_types), 1))")
    cur.execute("SELECT setval('users_id_seq', COALESCE((SELECT MAX(id) FROM users), 1))")


def run_migration_add_password_hash(cur):
    """Migration 2: Remove password column, add indexes."""
    cur.execute("ALTER TABLE users DROP COLUMN IF EXISTS password")
    cur.execute("CREATE INDEX IF NOT EXISTS users_login_key ON users (login)")
    cur.execute("CREATE INDEX IF NOT EXISTS users_barcode_key ON users (barcode)")


def run_migration_add_fulltext_search(cur):
    """Migration 3: Add full-text search capabilities."""
    cur.execute("ALTER TABLE items ADD COLUMN IF NOT EXISTS search_vector tsvector")
    
    cur.execute("""
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
        $$ LANGUAGE plpgsql
    """)
    
    cur.execute("DROP TRIGGER IF EXISTS items_search_vector_trigger ON items")
    cur.execute("""
        CREATE TRIGGER items_search_vector_trigger
            BEFORE INSERT OR UPDATE ON items
            FOR EACH ROW
            EXECUTE FUNCTION items_search_vector_update()
    """)
    
    cur.execute("CREATE INDEX IF NOT EXISTS items_search_vector_idx ON items USING GIN(search_vector)")
    
    cur.execute("""
        UPDATE items SET search_vector = 
            setweight(to_tsvector('french', COALESCE(title1, '')), 'A') ||
            setweight(to_tsvector('french', COALESCE(title2, '')), 'B') ||
            setweight(to_tsvector('french', COALESCE(keywords, '')), 'B') ||
            setweight(to_tsvector('french', COALESCE(abstract, '')), 'C') ||
            setweight(to_tsvector('french', COALESCE(subject, '')), 'C') ||
            setweight(to_tsvector('french', COALESCE(content, '')), 'D')
    """)


def run_migration_convert_dates_to_timestamptz(cur):
    """Migration 4: Convert date columns from INTEGER to TIMESTAMPTZ."""
    # Users table
    cur.execute("ALTER TABLE users ALTER COLUMN archived_date DROP DEFAULT")
    cur.execute("""
        ALTER TABLE users
            ALTER COLUMN crea_date TYPE TIMESTAMPTZ USING 
                CASE WHEN crea_date IS NULL OR crea_date = 0 THEN NULL ELSE to_timestamp(crea_date) END,
            ALTER COLUMN modif_date TYPE TIMESTAMPTZ USING 
                CASE WHEN modif_date IS NULL OR modif_date = 0 THEN NULL ELSE to_timestamp(modif_date) END,
            ALTER COLUMN issue_date TYPE TIMESTAMPTZ USING 
                CASE WHEN issue_date IS NULL OR issue_date = 0 THEN NULL ELSE to_timestamp(issue_date) END,
            ALTER COLUMN archived_date TYPE TIMESTAMPTZ USING 
                CASE WHEN archived_date IS NULL OR archived_date = 0 THEN NULL ELSE to_timestamp(archived_date) END
    """)
    
    # Items table
    cur.execute("""
        ALTER TABLE items
            ALTER COLUMN crea_date TYPE TIMESTAMPTZ USING 
                CASE WHEN crea_date IS NULL OR crea_date = 0 THEN NULL ELSE to_timestamp(crea_date) END,
            ALTER COLUMN modif_date TYPE TIMESTAMPTZ USING 
                CASE WHEN modif_date IS NULL OR modif_date = 0 THEN NULL ELSE to_timestamp(modif_date) END,
            ALTER COLUMN archived_timestamp TYPE TIMESTAMPTZ USING 
                CASE WHEN archived_timestamp IS NULL OR archived_timestamp = 0 THEN NULL ELSE to_timestamp(archived_timestamp) END
    """)
    
    # Remote items table
    cur.execute("""
        ALTER TABLE remote_items
            ALTER COLUMN crea_date TYPE TIMESTAMPTZ USING 
                CASE WHEN crea_date IS NULL OR crea_date = 0 THEN NULL ELSE to_timestamp(crea_date) END,
            ALTER COLUMN modif_date TYPE TIMESTAMPTZ USING 
                CASE WHEN modif_date IS NULL OR modif_date = 0 THEN NULL ELSE to_timestamp(modif_date) END,
            ALTER COLUMN archived_timestamp TYPE TIMESTAMPTZ USING 
                CASE WHEN archived_timestamp IS NULL OR archived_timestamp = 0 THEN NULL ELSE to_timestamp(archived_timestamp) END
    """)
    
    # Specimens table
    cur.execute("ALTER TABLE specimens ALTER COLUMN archive_date DROP DEFAULT")
    cur.execute("""
        ALTER TABLE specimens
            ALTER COLUMN crea_date TYPE TIMESTAMPTZ USING 
                CASE WHEN crea_date IS NULL OR crea_date = 0 THEN NULL ELSE to_timestamp(crea_date) END,
            ALTER COLUMN modif_date TYPE TIMESTAMPTZ USING 
                CASE WHEN modif_date IS NULL OR modif_date = 0 THEN NULL ELSE to_timestamp(modif_date) END,
            ALTER COLUMN archive_date TYPE TIMESTAMPTZ USING 
                CASE WHEN archive_date IS NULL OR archive_date = 0 THEN NULL ELSE to_timestamp(archive_date) END
    """)
    
    # Remote specimens table
    cur.execute("""
        ALTER TABLE remote_specimens
            ALTER COLUMN creation_date TYPE TIMESTAMPTZ USING 
                CASE WHEN creation_date IS NULL OR creation_date = 0 THEN NULL ELSE to_timestamp(creation_date) END,
            ALTER COLUMN modif_date TYPE TIMESTAMPTZ USING 
                CASE WHEN modif_date IS NULL OR modif_date = 0 THEN NULL ELSE to_timestamp(modif_date) END
    """)
    
    # Borrows table
    cur.execute("""
        ALTER TABLE borrows
            ALTER COLUMN date TYPE TIMESTAMPTZ USING to_timestamp(date),
            ALTER COLUMN renew_date TYPE TIMESTAMPTZ USING 
                CASE WHEN renew_date IS NULL OR renew_date = 0 THEN NULL ELSE to_timestamp(renew_date) END,
            ALTER COLUMN issue_date TYPE TIMESTAMPTZ USING 
                CASE WHEN issue_date IS NULL OR issue_date = 0 THEN NULL ELSE to_timestamp(issue_date) END,
            ALTER COLUMN returned_date TYPE TIMESTAMPTZ USING 
                CASE WHEN returned_date IS NULL OR returned_date = 0 THEN NULL ELSE to_timestamp(returned_date) END
    """)
    
    # Borrows archives table
    cur.execute("""
        ALTER TABLE borrows_archives
            ALTER COLUMN date TYPE TIMESTAMPTZ USING to_timestamp(date),
            ALTER COLUMN issue_date TYPE TIMESTAMPTZ USING 
                CASE WHEN issue_date IS NULL OR issue_date = 0 THEN NULL ELSE to_timestamp(issue_date) END,
            ALTER COLUMN returned_date TYPE TIMESTAMPTZ USING 
                CASE WHEN returned_date IS NULL OR returned_date = 0 THEN NULL ELSE to_timestamp(returned_date) END
    """)
    
    # Add NOT NULL constraint back
    cur.execute("ALTER TABLE borrows ALTER COLUMN date SET NOT NULL")
    cur.execute("ALTER TABLE borrows_archives ALTER COLUMN date SET NOT NULL")


def run_migration_rename_borrows_and_user_status(cur):
    """Migration 5: Rename borrows tables, add occupations and user status."""
    # Create occupations table
    cur.execute("""
        CREATE TABLE IF NOT EXISTS occupations (
            id SERIAL PRIMARY KEY,
            code VARCHAR(50) NOT NULL UNIQUE,
            label VARCHAR(255) NOT NULL,
            description VARCHAR(500),
            is_active BOOLEAN DEFAULT TRUE,
            sort_order INTEGER DEFAULT 0
        )
    """)
    
    cur.execute("""
        INSERT INTO occupations (code, label, sort_order) VALUES
            ('student', 'Étudiant', 1),
            ('teacher', 'Enseignant', 2),
            ('employee', 'Salarié', 3),
            ('self_employed', 'Indépendant', 4),
            ('retired', 'Retraité', 5),
            ('unemployed', 'Sans emploi', 6),
            ('other', 'Autre', 99)
        ON CONFLICT (code) DO NOTHING
    """)
    
    # Add password column (VARCHAR 255 for hash)
    cur.execute("""
        DO $$ 
        BEGIN
            IF NOT EXISTS (SELECT 1 FROM information_schema.columns 
                           WHERE table_name = 'users' AND column_name = 'password') THEN
                ALTER TABLE users ADD COLUMN password VARCHAR(255);
            ELSE
                ALTER TABLE users ALTER COLUMN password TYPE VARCHAR(255);
            END IF;
        END $$;
    """)
    
    # Add status and occupation_id columns
    cur.execute("ALTER TABLE users ADD COLUMN IF NOT EXISTS status SMALLINT DEFAULT 0")
    cur.execute("ALTER TABLE users ADD COLUMN IF NOT EXISTS occupation_id INTEGER")
    
    # Add foreign key constraint
    cur.execute("""
        DO $$
        BEGIN
            IF NOT EXISTS (SELECT 1 FROM information_schema.table_constraints 
                           WHERE constraint_name = 'fk_users_occupation' AND table_name = 'users') THEN
                ALTER TABLE users ADD CONSTRAINT fk_users_occupation 
                    FOREIGN KEY (occupation_id) REFERENCES occupations(id) ON DELETE SET NULL;
            END IF;
        END $$;
    """)
    
    cur.execute("CREATE INDEX IF NOT EXISTS idx_users_status ON users(status)")
    
    # Rename borrows tables
    cur.execute("ALTER TABLE IF EXISTS borrows RENAME TO loans")
    
    cur.execute("""
        DO $$
        BEGIN
            IF EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = 'borrows_archives')
               AND NOT EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = 'loans_archives') THEN
                ALTER TABLE borrows_archives RENAME TO loans_archives;
            END IF;
        END $$;
    """)
    
    cur.execute("ALTER TABLE IF EXISTS borrows_settings RENAME TO loans_settings")
    
    # Rename indexes
    cur.execute("ALTER INDEX IF EXISTS borrows_id_key RENAME TO loans_id_key")
    cur.execute("ALTER INDEX IF EXISTS borrows_user_id_key RENAME TO loans_user_id_key")
    cur.execute("ALTER INDEX IF EXISTS borrows_specimen_id_key RENAME TO loans_specimen_id_key")
    cur.execute("ALTER INDEX IF EXISTS borrows_pkey RENAME TO loans_pkey")
    
    cur.execute("""
        DO $$
        BEGIN
            IF EXISTS (SELECT 1 FROM pg_indexes WHERE indexname = 'borrows_archives_id_key')
               AND NOT EXISTS (SELECT 1 FROM pg_indexes WHERE indexname = 'loans_archives_id_key') THEN
                ALTER INDEX borrows_archives_id_key RENAME TO loans_archives_id_key;
            END IF;
            IF EXISTS (SELECT 1 FROM pg_indexes WHERE indexname = 'borrows_archives_item_id_key')
               AND NOT EXISTS (SELECT 1 FROM pg_indexes WHERE indexname = 'loans_archives_item_id_key') THEN
                ALTER INDEX borrows_archives_item_id_key RENAME TO loans_archives_item_id_key;
            END IF;
            IF EXISTS (SELECT 1 FROM pg_indexes WHERE indexname = 'borrows_archives_pkey')
               AND NOT EXISTS (SELECT 1 FROM pg_indexes WHERE indexname = 'loans_archives_pkey') THEN
                ALTER INDEX borrows_archives_pkey RENAME TO loans_archives_pkey;
            END IF;
        END $$;
    """)
    
    cur.execute("ALTER INDEX IF EXISTS borrows_settings_id_key RENAME TO loans_settings_id_key")
    cur.execute("ALTER INDEX IF EXISTS borrows_settings_media_type_key RENAME TO loans_settings_media_type_key")
    cur.execute("ALTER INDEX IF EXISTS borrows_settings_pkey RENAME TO loans_settings_pkey")
    
    # Rename sequences
    cur.execute("ALTER SEQUENCE IF EXISTS borrows_id_seq RENAME TO loans_id_seq")
    
    cur.execute("""
        DO $$
        BEGIN
            IF EXISTS (SELECT 1 FROM pg_sequences WHERE sequencename = 'borrows_archives_id_seq')
               AND NOT EXISTS (SELECT 1 FROM pg_sequences WHERE sequencename = 'loans_archives_id_seq') THEN
                ALTER SEQUENCE borrows_archives_id_seq RENAME TO loans_archives_id_seq;
            END IF;
        END $$;
    """)
    
    cur.execute("ALTER SEQUENCE IF EXISTS borrows_settings_id_seq RENAME TO loans_settings_id_seq")
    
    # Add user_id to loans_archives
    cur.execute("ALTER TABLE loans_archives ADD COLUMN IF NOT EXISTS user_id INTEGER")
    
    # Add comments
    cur.execute("COMMENT ON TABLE occupations IS 'Reference table for user occupation codes'")
    cur.execute("COMMENT ON COLUMN users.status IS 'User status: 0=active, 1=blocked, 2=deleted'")
    cur.execute("COMMENT ON COLUMN users.occupation_id IS 'Foreign key to occupations table'")
    cur.execute("COMMENT ON TABLE loans IS 'Active loans (renamed from borrows)'")
    cur.execute("COMMENT ON TABLE loans_archives IS 'Archived/returned loans (renamed from borrows_archives)'")
    cur.execute("COMMENT ON TABLE loans_settings IS 'Loan duration settings by media type (renamed from borrows_settings)'")


def run_migration_add_status_to_items_specimens(cur):
    """Migration 6: Add lifecycle_status to items and specimens."""
    cur.execute("ALTER TABLE items ADD COLUMN IF NOT EXISTS lifecycle_status SMALLINT DEFAULT 0 NOT NULL")
    cur.execute("ALTER TABLE items ADD COLUMN IF NOT EXISTS archived_date TIMESTAMPTZ")
    cur.execute("UPDATE items SET lifecycle_status = 2, archived_date = NOW() WHERE is_archive = 1 AND lifecycle_status = 0")
    
    cur.execute("ALTER TABLE specimens ADD COLUMN IF NOT EXISTS lifecycle_status SMALLINT DEFAULT 0 NOT NULL")
    cur.execute("UPDATE specimens SET lifecycle_status = 2, archive_date = NOW() WHERE is_archive = 1 AND lifecycle_status = 0")
    
    cur.execute("ALTER TABLE remote_items ADD COLUMN IF NOT EXISTS lifecycle_status SMALLINT DEFAULT 0 NOT NULL")
    cur.execute("ALTER TABLE remote_specimens ADD COLUMN IF NOT EXISTS lifecycle_status SMALLINT DEFAULT 0 NOT NULL")
    
    cur.execute("CREATE INDEX IF NOT EXISTS items_lifecycle_status_idx ON items (lifecycle_status)")
    cur.execute("CREATE INDEX IF NOT EXISTS specimens_lifecycle_status_idx ON specimens (lifecycle_status)")


def run_migration_add_language_to_users(cur):
    """Migration 7: Add language column to users."""
    cur.execute("ALTER TABLE users ADD COLUMN IF NOT EXISTS language VARCHAR(5) DEFAULT 'fr'")


def run_migration_email_login_constraints(cur):
    """Migration 8: Add unique constraint on login (partial, will be superseded)."""
    cur.execute("""
        UPDATE users u1 
        SET login = CONCAT(login, '_', id) 
        WHERE login IS NOT NULL 
          AND login != ''
          AND EXISTS (
            SELECT 1 FROM users u2 
            WHERE u2.login = u1.login AND u2.id < u1.id
          )
    """)
    cur.execute("CREATE UNIQUE INDEX IF NOT EXISTS users_login_unique ON users (login) WHERE login IS NOT NULL AND login != ''")


def run_migration_remove_sex_id_from_users(cur):
    """Migration 9: Remove sex_id from users and loans_archives."""
    cur.execute("ALTER TABLE users DROP COLUMN IF EXISTS sex_id")
    cur.execute("ALTER TABLE loans_archives DROP COLUMN IF EXISTS sex_id")


def run_migration_revert_email_constraints_use_login(cur):
    """Migration 10: Make login required and unique, email optional."""
    cur.execute("ALTER TABLE users ALTER COLUMN email DROP NOT NULL")
    cur.execute("ALTER TABLE users DROP CONSTRAINT IF EXISTS users_email_unique")
    
    cur.execute("UPDATE users SET login = CONCAT('user_', id) WHERE login IS NULL OR login = ''")
    cur.execute("ALTER TABLE users ALTER COLUMN login SET NOT NULL")
    
    cur.execute("DROP INDEX IF EXISTS users_login_unique")
    cur.execute("ALTER TABLE users ADD CONSTRAINT users_login_unique UNIQUE (login)")
    
    cur.execute("CREATE INDEX IF NOT EXISTS users_email_idx ON users (email) WHERE email IS NOT NULL")


def reset_target_database(conn, target_db_url):
    """Reset (drop) all tables in target database and recreate them."""
    print("Resetting target database...")
    print("  WARNING: This will DROP ALL TABLES in the target database!")
    
    cur = conn.cursor()
    
    # Tables in reverse dependency order
    # Support both old (borrows*) and new (loans*) table names
    tables = [
        'loans_archives',
        'borrows_archives',
        'loans',
        'borrows',
        'specimens',
        'items',
        'remote_specimens',
        'remote_items',
        'loans_settings',
        'borrows_settings',
        'z3950servers',
        'fees',
        'users',
        'authors',
        'editions',
        'collections',
        'series',
        'sources',
        'occupations',
        'account_types',
        # Also drop SQLx migration tracking table to force re-run
        '_sqlx_migrations',
    ]
    
    for table in tables:
        try:
            cur.execute(f"DROP TABLE IF EXISTS {table} CASCADE")
            print(f"  Dropped {table}")
        except Exception as e:
            # Table might not exist
            conn.rollback()
    
    # Drop functions and triggers
    try:
        cur.execute("DROP FUNCTION IF EXISTS items_search_vector_update() CASCADE")
    except:
        pass
    
    conn.commit()
    print("  Database reset complete")
    print()
    
    # Run all schema migrations
    if run_schema_migrations(conn):
        print()
        return True
    else:
        print()
        print("  ERROR: Failed to recreate schema")
        return False


def migrate_account_types(source_conn, target_conn):
    """Migrate account_types table."""
    print("Migrating account_types...")
    
    source_cur = source_conn.cursor()
    target_cur = target_conn.cursor()
    
    source_cur.execute("""
        SELECT id, name, items_rights, users_rights, loans_rights, 
               items_archive_rights, borrows_rights, settings_rights
        FROM account_types
    """)
    rows = source_cur.fetchall()
    
    for row in rows:
        target_cur.execute("""
            INSERT INTO account_types (id, name, items_rights, users_rights, loans_rights, 
                                       items_archive_rights, borrows_rights, settings_rights)
            VALUES (%s, %s, %s, %s, %s, %s, %s, %s)
            ON CONFLICT (id) DO UPDATE SET
                name = EXCLUDED.name,
                items_rights = EXCLUDED.items_rights,
                users_rights = EXCLUDED.users_rights
        """, row)
    
    target_conn.commit()
    print(f"  Migrated {len(rows)} account types")


def get_occupation_mapping(target_conn):
    """Get mapping of occupation text to occupation_id."""
    cur = target_conn.cursor()
    
    # Check if occupations table exists
    cur.execute("""
        SELECT EXISTS (
            SELECT FROM information_schema.tables 
            WHERE table_name = 'occupations'
        )
    """)
    if not cur.fetchone()[0]:
        return {}
    
    cur.execute("SELECT id, code, label FROM occupations")
    occupations = cur.fetchall()
    
    # Build mapping (lowercase for case-insensitive matching)
    mapping = {}
    for occ_id, code, label in occupations:
        mapping[code.lower()] = occ_id
        mapping[label.lower()] = occ_id
    
    return mapping


def migrate_users(source_conn, target_conn, hash_passwords=False, use_timestamptz=False):
    """Migrate users table with optional password hashing."""
    print("Migrating users...")
    
    if hash_passwords:
        if not ARGON2_AVAILABLE:
            print("  ERROR: argon2-cffi required for password hashing")
            print("  Install with: pip install argon2-cffi")
            sys.exit(1)
        print("  Password hashing enabled (Argon2)")
        hasher = PasswordHasher()
    else:
        print("  Warning: Passwords will be migrated as plaintext")
        hasher = None
    
    source_cur = source_conn.cursor()
    target_cur = target_conn.cursor()
    
    # Get occupation mapping
    occupation_mapping = get_occupation_mapping(target_conn)
    
    # Check which columns exist in target users table
    target_cur.execute("""
        SELECT column_name FROM information_schema.columns 
        WHERE table_name = 'users' AND column_name IN ('occupation_id', 'status', 'password', 'password_hash', 'language')
    """)
    existing_columns = {row[0] for row in target_cur.fetchall()}
    has_occupation_id = 'occupation_id' in existing_columns
    has_status = 'status' in existing_columns
    has_password = 'password' in existing_columns
    has_password_hash = 'password_hash' in existing_columns
    has_language = 'language' in existing_columns
    
    # Determine password column name (new schema uses 'password' VARCHAR(255) for hash storage)
    # Migration 20260104000002 removes password, but 20260105000002 adds it back as VARCHAR(255) for hash
    if has_password:
        password_column = 'password'  # Stores hash in new schema
    elif has_password_hash:
        password_column = 'password_hash'
    else:
        password_column = None
        print("  WARNING: Target database has no 'password' or 'password_hash' column - passwords will not be migrated")
        print("  Run 'sqlx migrate run' first to create the schema")
    
    # Select columns explicitly in the right order
    source_cur.execute("""
        SELECT id, login, password, firstname, lastname, email,
               addr_street, addr_zip_code, addr_city, phone, sex_id,
               account_type_id, subscription_type_id, group_id, barcode,
               notes, occupation, crea_date, modif_date, issue_date,
               birthdate, archived_date, public_type
        FROM users
    """)
    users = source_cur.fetchall()
    
    # Track logins to detect duplicates and ensure uniqueness
    # Migration 20260105000007: login is required (NOT NULL) and unique
    login_map = {}  # login_lower -> list of (user_id, original_login)
    login_placeholder_count = 0
    
    for user in users:
        user_id = user[0]
        login = user[1]  # login is at index 1
        
        # Handle NULL or empty login (required by new schema - NOT NULL constraint)
        if not login or (isinstance(login, str) and login.strip() == ''):
            # Generate placeholder login (migration 20260105000007 does this too)
            login = f'user_{user_id}'
            login_placeholder_count += 1
        
        # Normalize login (trim whitespace)
        login = login.strip() if isinstance(login, str) else str(login)
        login_lower = login.lower()
        
        if login_lower not in login_map:
            login_map[login_lower] = []
        login_map[login_lower].append((user_id, login))
    
    # Build login mapping: user_id -> final_login (with duplicates resolved)
    # Migration 20260105000005 and 20260105000007: ensure unique logins
    user_login_map = {}
    for login_lower, user_list in login_map.items():
        if len(user_list) > 1:
            # Duplicate logins: make them unique by appending user_id
            # This matches the behavior in migration 20260105000005
            for user_id, original_login in user_list:
                user_login_map[user_id] = f'{original_login}_{user_id}'
        else:
            # Unique login: use as-is
            user_id, original_login = user_list[0]
            user_login_map[user_id] = original_login
    
    hashed_count = 0
    occupation_mapped = 0
    
    for user in users:
        user_list = list(user)
        # Source indices: id=0, login=1, password=2, firstname=3, lastname=4, email=5,
        # addr_street=6, addr_zip_code=7, addr_city=8, phone=9, sex_id=10,
        # account_type_id=11, subscription_type_id=12, group_id=13, barcode=14,
        # notes=15, occupation=16, crea_date=17, modif_date=18, issue_date=19,
        # birthdate=20, archived_date=21, public_type=22
        
        original_password = user_list[2]  # password is at index 2
        occupation_text = user_list[16]  # occupation is at index 16
        user_id = user_list[0]
        
        # Get the final login (with duplicates resolved)
        login = user_login_map.get(user_id, user_list[1])
        if not login or (isinstance(login, str) and login.strip() == ''):
            login = f'user_{user_id}'
            # Note: login_placeholder_count already incremented in first loop
        
        # Normalize login (trim whitespace)
        login = login.strip() if isinstance(login, str) else str(login)
        
        # Email can be NULL (migration 20260105000007: email is optional)
        email = user_list[5] if user_list[5] and (isinstance(user_list[5], str) and user_list[5].strip() != '') else None
        
        # Hash the password
        password = None
        if hash_passwords and original_password:
            # Check if already hashed (starts with $argon2)
            if not original_password.startswith('$argon2'):
                password = hash_password(original_password, hasher)
                hashed_count += 1
            else:
                password = original_password
        
        # Map occupation text to occupation_id
        occupation_id = None
        if has_occupation_id and occupation_text and occupation_mapping:
            occupation_id = occupation_mapping.get(occupation_text.lower().strip())
            if occupation_id:
                occupation_mapped += 1
        
        # Convert date columns if target uses TIMESTAMPTZ
        # Indices: crea_date=17, modif_date=18, issue_date=19, archived_date=21
        crea_date = convert_date(user_list[17], use_timestamptz)
        modif_date = convert_date(user_list[18], use_timestamptz)
        issue_date = convert_date(user_list[19], use_timestamptz)
        archived_date = convert_date(user_list[21], use_timestamptz)
        
        # Determine status (0=active by default, 2=deleted if archived)
        status = 0
        if archived_date is not None:
            status = 2  # deleted
        
        # Build values list
        # Migration 20260105000006: sex_id removed from users
        # Migration 20260105000004: language added to users
        # Migration 20260105000002: password column stores hash (VARCHAR 255)
        # Migration 20260105000007: login is NOT NULL and unique, email is optional
        
        # Base values (without password, occupation_id, status, language)
        base_values = [
            user_list[0],   # id
            login,          # login (required, NOT NULL, unique - migration 20260105000007)
            user_list[3],   # firstname
            user_list[4],   # lastname
            email,          # email (optional, can be NULL - migration 20260105000007)
            user_list[6],   # addr_street
            user_list[7],   # addr_zip_code
            user_list[8],   # addr_city
            user_list[9],   # phone
            user_list[11],  # account_type_id
            user_list[12],  # subscription_type_id
            user_list[13],  # group_id
            user_list[14],  # barcode
            user_list[15],  # notes
            user_list[16],  # occupation
            crea_date,      # crea_date
            modif_date,     # modif_date
            issue_date,     # issue_date
            user_list[20],  # birthdate
            archived_date,  # archived_date
            user_list[22],  # public_type
        ]
        
        # Build dynamic columns and values based on target schema
        columns = [
            "id", "login", "firstname", "lastname", "email",
            "addr_street", "addr_zip_code", "addr_city", "phone",
            "account_type_id", "subscription_type_id", "group_id", "barcode",
            "notes", "occupation", "crea_date", "modif_date", "issue_date",
            "birthdate", "archived_date", "public_type"
        ]
        values = base_values[:]
        
        # Note: sex_id removed in migration 20260105000006 - do not include it
        
        update_cols = ["login", "firstname", "lastname"]
        
        if password_column:
            columns.append(password_column)
            values.append(password)
            update_cols.append(password_column)
        
        if has_occupation_id:
            columns.append("occupation_id")
            values.append(occupation_id)
            update_cols.append("occupation_id")
        
        if has_status:
            columns.append("status")
            values.append(status)
            update_cols.append("status")
        
        # Add language column if it exists (migration 20260105000004)
        if has_language:
            columns.append("language")
            values.append('fr')  # Default to French
            # Don't update language on conflict (preserve existing preference)
        
        placeholders = ", ".join(["%s"] * len(values))
        columns_str = ", ".join(columns)
        update_str = ", ".join([f"{col} = EXCLUDED.{col}" for col in update_cols])
        
        # Use ON CONFLICT with login unique constraint handling
        # Migration 20260105000007: login has UNIQUE constraint
        try:
            target_cur.execute(f"""
                INSERT INTO users ({columns_str})
                VALUES ({placeholders})
                ON CONFLICT (id) DO UPDATE SET {update_str}
            """, values)
        except IntegrityError as e:
            # Handle unique constraint violation on login
            if 'users_login_unique' in str(e) or 'login' in str(e).lower():
                # Login conflict - append user_id to make it unique
                login = f'{login}_{user_id}'
                # Update login in values
                login_idx = columns.index("login")
                values[login_idx] = login
                # Retry insert
                target_cur.execute(f"""
                    INSERT INTO users ({columns_str})
                    VALUES ({placeholders})
                    ON CONFLICT (id) DO UPDATE SET {update_str}
                """, values)
            else:
                raise
    
    target_conn.commit()
    
    msg_parts = [f"Migrated {len(users)} users"]
    if hash_passwords:
        msg_parts.append(f"{hashed_count} passwords hashed")
    if occupation_mapped > 0:
        msg_parts.append(f"{occupation_mapped} occupations mapped")
    if login_placeholder_count > 0:
        msg_parts.append(f"{login_placeholder_count} placeholder logins generated")
    print(f"  {', '.join(msg_parts)}")


def migrate_authors(source_conn, target_conn):
    """Migrate authors table."""
    print("Migrating authors...")
    
    source_cur = source_conn.cursor()
    target_cur = target_conn.cursor()
    
    source_cur.execute("SELECT id, key, lastname, firstname, bio, notes FROM authors")
    authors = source_cur.fetchall()
    
    for author in authors:
        target_cur.execute("""
            INSERT INTO authors (id, key, lastname, firstname, bio, notes)
            VALUES (%s, %s, %s, %s, %s, %s)
            ON CONFLICT (id) DO UPDATE SET
                lastname = EXCLUDED.lastname,
                firstname = EXCLUDED.firstname
        """, author)
    
    target_conn.commit()
    print(f"  Migrated {len(authors)} authors")


def migrate_editions(source_conn, target_conn):
    """Migrate editions table."""
    print("Migrating editions...")
    
    source_cur = source_conn.cursor()
    target_cur = target_conn.cursor()
    
    source_cur.execute("SELECT id, key, name, place, notes FROM editions")
    rows = source_cur.fetchall()
    
    for row in rows:
        target_cur.execute("""
            INSERT INTO editions (id, key, name, place, notes)
            VALUES (%s, %s, %s, %s, %s)
            ON CONFLICT (id) DO NOTHING
        """, row)
    
    target_conn.commit()
    print(f"  Migrated {len(rows)} editions")


def migrate_collections(source_conn, target_conn):
    """Migrate collections table."""
    print("Migrating collections...")
    
    source_cur = source_conn.cursor()
    target_cur = target_conn.cursor()
    
    source_cur.execute("SELECT id, key, title1, title2, title3, issn FROM collections")
    rows = source_cur.fetchall()
    
    for row in rows:
        target_cur.execute("""
            INSERT INTO collections (id, key, title1, title2, title3, issn)
            VALUES (%s, %s, %s, %s, %s, %s)
            ON CONFLICT (id) DO NOTHING
        """, row)
    
    target_conn.commit()
    print(f"  Migrated {len(rows)} collections")


def migrate_series(source_conn, target_conn):
    """Migrate series table."""
    print("Migrating series...")
    
    source_cur = source_conn.cursor()
    target_cur = target_conn.cursor()
    
    source_cur.execute("SELECT id, key, name FROM series")
    rows = source_cur.fetchall()
    
    for row in rows:
        target_cur.execute("""
            INSERT INTO series (id, key, name)
            VALUES (%s, %s, %s)
            ON CONFLICT (id) DO NOTHING
        """, row)
    
    target_conn.commit()
    print(f"  Migrated {len(rows)} series")


def migrate_sources(source_conn, target_conn):
    """Migrate sources table."""
    print("Migrating sources...")
    
    source_cur = source_conn.cursor()
    target_cur = target_conn.cursor()
    
    source_cur.execute("SELECT id, key, name FROM sources")
    rows = source_cur.fetchall()
    
    for row in rows:
        target_cur.execute("""
            INSERT INTO sources (id, key, name)
            VALUES (%s, %s, %s)
            ON CONFLICT (id) DO NOTHING
        """, row)
    
    target_conn.commit()
    print(f"  Migrated {len(rows)} sources")


def migrate_items(source_conn, target_conn, use_timestamptz=False):
    """Migrate items table."""
    print("Migrating items...")
    
    source_cur = source_conn.cursor()
    target_cur = target_conn.cursor()
    
    # Check if target has lifecycle_status column
    target_cur.execute("""
        SELECT column_name FROM information_schema.columns 
        WHERE table_name = 'items' AND column_name = 'lifecycle_status'
    """)
    has_lifecycle_status = target_cur.fetchone() is not None
    
    # Check if target has archived_date column
    target_cur.execute("""
        SELECT column_name FROM information_schema.columns 
        WHERE table_name = 'items' AND column_name = 'archived_date'
    """)
    has_archived_date = target_cur.fetchone() is not None
    
    source_cur.execute("SELECT COUNT(*) FROM items")
    count = source_cur.fetchone()[0]
    
    batch_size = 1000
    offset = 0
    
    while offset < count:
        source_cur.execute(f"""
            SELECT id, media_type, identification, price, barcode, dewey,
                   publication_date, lang, lang_orig, title1, title2, title3, title4,
                   author1_ids, author1_functions, author2_ids, author2_functions,
                   author3_ids, author3_functions, serie_id, serie_vol_number,
                   collection_id, collection_number_sub, collection_vol_number,
                   source_id, source_date, source_norme, genre, subject, public_type,
                   edition_id, edition_date, nb_pages, format, content, addon,
                   abstract, notes, keywords, nb_specimens, state, is_archive,
                   archived_timestamp, is_valid, crea_date, modif_date
            FROM items
            ORDER BY id
            LIMIT {batch_size} OFFSET {offset}
        """)
        items = source_cur.fetchall()
        
        for item in items:
            item_list = list(item)
            # Convert date columns if target uses TIMESTAMPTZ
            # Indices: archived_timestamp=42, crea_date=44, modif_date=45
            item_list[42] = convert_date(item_list[42], use_timestamptz)  # archived_timestamp
            item_list[44] = convert_date(item_list[44], use_timestamptz)  # crea_date
            item_list[45] = convert_date(item_list[45], use_timestamptz)  # modif_date
            
            # Calculate lifecycle_status from is_archive (migration 20260105000003)
            is_archive = item_list[41]  # is_archive index
            lifecycle_status = 2 if is_archive == 1 else 0  # 0=Active, 2=Deleted
            
            # Calculate archived_date (migration 20260105000003: archived_date TIMESTAMPTZ)
            # archived_timestamp is already converted to TIMESTAMPTZ if needed
            archived_date = item_list[42] if is_archive == 1 else None  # Use archived_timestamp
            
            if has_lifecycle_status and has_archived_date:
                target_cur.execute("""
                    INSERT INTO items (
                        id, media_type, identification, price, barcode, dewey,
                        publication_date, lang, lang_orig, title1, title2, title3, title4,
                        author1_ids, author1_functions, author2_ids, author2_functions,
                        author3_ids, author3_functions, serie_id, serie_vol_number,
                        collection_id, collection_number_sub, collection_vol_number,
                        source_id, source_date, source_norme, genre, subject, public_type,
                        edition_id, edition_date, nb_pages, format, content, addon,
                        abstract, notes, keywords, nb_specimens, state, is_archive,
                        archived_timestamp, is_valid, crea_date, modif_date,
                        lifecycle_status, archived_date
                    ) VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s,
                              %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s,
                              %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s,
                              %s, %s)
                    ON CONFLICT (id) DO NOTHING
                """, item_list + [lifecycle_status, archived_date])
            else:
                target_cur.execute("""
                    INSERT INTO items (
                        id, media_type, identification, price, barcode, dewey,
                        publication_date, lang, lang_orig, title1, title2, title3, title4,
                        author1_ids, author1_functions, author2_ids, author2_functions,
                        author3_ids, author3_functions, serie_id, serie_vol_number,
                        collection_id, collection_number_sub, collection_vol_number,
                        source_id, source_date, source_norme, genre, subject, public_type,
                        edition_id, edition_date, nb_pages, format, content, addon,
                        abstract, notes, keywords, nb_specimens, state, is_archive,
                        archived_timestamp, is_valid, crea_date, modif_date
                    ) VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s,
                              %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s,
                              %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s)
                    ON CONFLICT (id) DO NOTHING
                """, item_list)
        
        target_conn.commit()
        offset += batch_size
        print(f"  Migrated {min(offset, count)}/{count} items")
    
    print(f"  Completed: {count} items")


def migrate_specimens(source_conn, target_conn, use_timestamptz=False):
    """Migrate specimens table."""
    print("Migrating specimens...")
    
    source_cur = source_conn.cursor()
    target_cur = target_conn.cursor()
    
    # Check if target has lifecycle_status column
    target_cur.execute("""
        SELECT column_name FROM information_schema.columns 
        WHERE table_name = 'specimens' AND column_name = 'lifecycle_status'
    """)
    has_lifecycle_status = target_cur.fetchone() is not None
    
    source_cur.execute("SELECT COUNT(*) FROM specimens")
    count = source_cur.fetchone()[0]
    
    batch_size = 1000
    offset = 0
    
    while offset < count:
        source_cur.execute(f"""
            SELECT id, id_item, source_id, identification, cote, place,
                   status, codestat, notes, price, modif_date, is_archive,
                   archive_date, crea_date
            FROM specimens
            ORDER BY id
            LIMIT {batch_size} OFFSET {offset}
        """)
        specimens = source_cur.fetchall()
        
        for specimen in specimens:
            specimen_list = list(specimen)
            # Convert date columns if target uses TIMESTAMPTZ
            # Indices: modif_date=10, archive_date=12, crea_date=13
            specimen_list[10] = convert_date(specimen_list[10], use_timestamptz)  # modif_date
            specimen_list[12] = convert_date(specimen_list[12], use_timestamptz)  # archive_date
            specimen_list[13] = convert_date(specimen_list[13], use_timestamptz)  # crea_date
            
            # Calculate lifecycle_status from is_archive
            is_archive = specimen_list[11]  # is_archive index
            lifecycle_status = 2 if is_archive == 1 else 0  # 0=Active, 2=Deleted
            
            if has_lifecycle_status:
                target_cur.execute("""
                    INSERT INTO specimens (
                        id, id_item, source_id, identification, cote, place,
                        status, codestat, notes, price, modif_date, is_archive,
                        archive_date, crea_date, lifecycle_status
                    ) VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s)
                    ON CONFLICT (id) DO NOTHING
                """, specimen_list + [lifecycle_status])
            else:
                target_cur.execute("""
                    INSERT INTO specimens (
                        id, id_item, source_id, identification, cote, place,
                        status, codestat, notes, price, modif_date, is_archive,
                        archive_date, crea_date
                    ) VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s)
                    ON CONFLICT (id) DO NOTHING
                """, specimen_list)
        
        target_conn.commit()
        offset += batch_size
        print(f"  Migrated {min(offset, count)}/{count} specimens")
    
    print(f"  Completed: {count} specimens")


def get_loans_table_name(target_conn):
    """Detect if target uses 'loans' or 'borrows' table name."""
    cur = target_conn.cursor()
    cur.execute("""
        SELECT EXISTS (
            SELECT FROM information_schema.tables 
            WHERE table_name = 'loans'
        )
    """)
    return 'loans' if cur.fetchone()[0] else 'borrows'


def migrate_loans(source_conn, target_conn, use_timestamptz=False):
    """Migrate borrows/loans table. Returned loans go to archives."""
    
    loans_table = get_loans_table_name(target_conn)
    archives_table = 'loans_archives' if loans_table == 'loans' else 'borrows_archives'
    
    print(f"Migrating loans (source: borrows -> target: {loans_table})...")
    
    source_cur = source_conn.cursor()
    target_cur = target_conn.cursor()
    
    # Check if archives table has user_id column (new schema)
    target_cur.execute("""
        SELECT EXISTS (
            SELECT FROM information_schema.columns 
            WHERE table_name = %s AND column_name = 'user_id'
        )
    """, (archives_table,))
    archives_has_user_id = target_cur.fetchone()[0]
    
    # Migration 20260105000006: sex_id removed from loans_archives
    # Do not check for sex_id - it's been removed
    
    # Get user info for archiving (occupation, city, account_type, etc.)
    # Note: sex_id removed from users table too (migration 20260105000006)
    source_cur.execute("""
        SELECT id, occupation, addr_city, account_type_id, public_type
        FROM users
    """)
    user_info = {row[0]: row[1:] for row in source_cur.fetchall()}
    
    source_cur.execute("""
        SELECT id, user_id, specimen_id, item_id, date, renew_date,
               nb_renews, issue_date, notes, returned_date
        FROM borrows
    """)
    borrows = source_cur.fetchall()
    
    active_count = 0
    archived_count = 0
    
    for borrow in borrows:
        borrow_list = list(borrow)
        user_id = borrow_list[1]
        returned_date = borrow_list[9]
        
        # Convert date columns if target uses TIMESTAMPTZ
        # Indices: date=4, renew_date=5, issue_date=7, returned_date=9
        borrow_list[4] = convert_date(borrow_list[4], use_timestamptz)  # date
        borrow_list[5] = convert_date(borrow_list[5], use_timestamptz)  # renew_date
        borrow_list[7] = convert_date(borrow_list[7], use_timestamptz)  # issue_date
        borrow_list[9] = convert_date(borrow_list[9], use_timestamptz)  # returned_date
        
        if returned_date is not None and returned_date != 0:
            # Returned loan -> goes to archives
            # Migration 20260105000006: sex_id removed from loans_archives
            user_data = user_info.get(user_id, (None, None, None, None))
            occupation, addr_city, account_type_id, public_type = user_data
            
            # Build columns and values dynamically based on schema
            # Migration 20260105000002: user_id added to loans_archives
            if archives_has_user_id:
                # New schema with user_id column (migration 20260105000002)
                columns = [
                    "id", "user_id", "item_id", "specimen_id", "date", "nb_renews", "issue_date",
                    "returned_date", "notes", "borrower_public_type", "occupation",
                    "addr_city", "account_type_id"
                ]
                values = [
                    borrow_list[0],  # id
                    user_id,         # user_id (migration 20260105000002)
                    borrow_list[3],  # item_id
                    borrow_list[2],  # specimen_id
                    borrow_list[4],  # date
                    borrow_list[6],  # nb_renews
                    borrow_list[7],  # issue_date
                    borrow_list[9],  # returned_date
                    borrow_list[8],  # notes
                    public_type,     # borrower_public_type
                    occupation,      # occupation
                    addr_city,       # addr_city
                    account_type_id, # account_type_id
                ]
                # Note: sex_id removed in migration 20260105000006 - do not include
            else:
                # Old schema without user_id column
                columns = [
                    "id", "item_id", "specimen_id", "date", "nb_renews", "issue_date",
                    "returned_date", "notes", "borrower_public_type", "occupation",
                    "addr_city", "account_type_id"
                ]
                values = [
                    borrow_list[0],  # id
                    borrow_list[3],  # item_id
                    borrow_list[2],  # specimen_id
                    borrow_list[4],  # date
                    borrow_list[6],  # nb_renews
                    borrow_list[7],  # issue_date
                    borrow_list[9],  # returned_date
                    borrow_list[8],  # notes
                    public_type,     # borrower_public_type
                    occupation,      # occupation
                    addr_city,       # addr_city
                    account_type_id, # account_type_id
                ]
                # Note: sex_id removed in migration 20260105000006 - do not include
            
            # Execute insert
            columns_str = ", ".join(columns)
            placeholders = ", ".join(["%s"] * len(values))
            target_cur.execute(f"""
                INSERT INTO {archives_table} ({columns_str})
                VALUES ({placeholders})
                ON CONFLICT (id) DO NOTHING
            """, values)
            archived_count += 1
        else:
            # Active loan -> stays in loans table
            target_cur.execute(f"""
                INSERT INTO {loans_table} (
                    id, user_id, specimen_id, item_id, date, renew_date,
                    nb_renews, issue_date, notes, returned_date
                ) VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s)
                ON CONFLICT (id) DO NOTHING
            """, borrow_list)
            active_count += 1
    
    target_conn.commit()
    print(f"  Migrated {len(borrows)} loans ({active_count} active, {archived_count} archived)")


def migrate_loans_archives(source_conn, target_conn, use_timestamptz=False):
    """Migrate borrows_archives/loans_archives table."""
    
    archives_table = 'loans_archives' if get_loans_table_name(target_conn) == 'loans' else 'borrows_archives'
    
    print(f"Migrating loans archives (source: borrows_archives -> target: {archives_table})...")
    
    source_cur = source_conn.cursor()
    target_cur = target_conn.cursor()
    
    # Migration 20260105000006: sex_id removed from loans_archives
    # Check if archives table has user_id column (migration 20260105000002)
    target_cur.execute("""
        SELECT EXISTS (
            SELECT FROM information_schema.columns 
            WHERE table_name = %s AND column_name = 'user_id'
        )
    """, (archives_table,))
    archives_has_user_id = target_cur.fetchone()[0]
    
    # Source may have sex_id, but we won't migrate it (removed in target)
    source_cur.execute("""
        SELECT id, item_id, specimen_id, date, nb_renews, issue_date,
               returned_date, notes, borrower_public_type, occupation,
               addr_city, sex_id, account_type_id
        FROM borrows_archives
    """)
    rows = source_cur.fetchall()
    
    for row in rows:
        row_list = list(row)
        # Convert date columns if target uses TIMESTAMPTZ
        # Indices: date=3, issue_date=5, returned_date=6
        row_list[3] = convert_date(row_list[3], use_timestamptz)  # date
        row_list[5] = convert_date(row_list[5], use_timestamptz)  # issue_date
        row_list[6] = convert_date(row_list[6], use_timestamptz)  # returned_date
        
        # Build columns and values dynamically
        # Source indices: id=0, item_id=1, specimen_id=2, date=3, nb_renews=4,
        # issue_date=5, returned_date=6, notes=7, borrower_public_type=8,
        # occupation=9, addr_city=10, sex_id=11 (not migrated), account_type_id=12
        columns = [
            "id", "item_id", "specimen_id", "date", "nb_renews", "issue_date",
            "returned_date", "notes", "borrower_public_type", "occupation",
            "addr_city", "account_type_id"
        ]
        values = [
            row_list[0],   # id
            row_list[1],   # item_id
            row_list[2],   # specimen_id
            row_list[3],   # date
            row_list[4],   # nb_renews
            row_list[5],   # issue_date
            row_list[6],   # returned_date
            row_list[7],   # notes
            row_list[8],   # borrower_public_type
            row_list[9],   # occupation
            row_list[10],  # addr_city
            row_list[12],  # account_type_id
        ]
        
        # Migration 20260105000006: sex_id removed - do not include it
        # Migration 20260105000002: user_id may be added, but we don't have it in source archives
        # (user_id is only added when migrating from active loans, not from existing archives)
        
        columns_str = ", ".join(columns)
        placeholders = ", ".join(["%s"] * len(values))
        
        target_cur.execute(f"""
            INSERT INTO {archives_table} ({columns_str})
            VALUES ({placeholders})
            ON CONFLICT (id) DO NOTHING
        """, values)
    
    target_conn.commit()
    print(f"  Migrated {len(rows)} loans archives")


def migrate_settings(source_conn, target_conn):
    """Migrate settings tables."""
    
    settings_table = 'loans_settings' if get_loans_table_name(target_conn) == 'loans' else 'borrows_settings'
    
    print(f"Migrating loan settings (source: borrows_settings -> target: {settings_table})...")
    
    source_cur = source_conn.cursor()
    target_cur = target_conn.cursor()
    
    source_cur.execute("""
        SELECT id, media_type, nb_max, nb_renews, duration, notes, account_type_id
        FROM borrows_settings
    """)
    settings = source_cur.fetchall()
    
    for setting in settings:
        target_cur.execute(f"""
            INSERT INTO {settings_table} (id, media_type, nb_max, nb_renews, duration, notes, account_type_id)
            VALUES (%s, %s, %s, %s, %s, %s, %s)
            ON CONFLICT (id) DO NOTHING
        """, setting)
    
    target_conn.commit()
    print(f"  Migrated {len(settings)} loan settings")


def migrate_z3950servers(source_conn, target_conn):
    """Migrate Z39.50 servers."""
    print("Migrating z3950servers...")
    
    source_cur = source_conn.cursor()
    target_cur = target_conn.cursor()
    
    source_cur.execute("""
        SELECT id, address, port, name, description, activated, login, password, database, format
        FROM z3950servers
    """)
    servers = source_cur.fetchall()
    
    for server in servers:
        target_cur.execute("""
            INSERT INTO z3950servers (id, address, port, name, description, activated, login, password, database, format)
            VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s)
            ON CONFLICT (id) DO NOTHING
        """, server)
    
    target_conn.commit()
    print(f"  Migrated {len(servers)} Z39.50 servers")


def migrate_fees(source_conn, target_conn):
    """Migrate fees table."""
    print("Migrating fees...")
    
    source_cur = source_conn.cursor()
    target_cur = target_conn.cursor()
    
    source_cur.execute('SELECT id, "desc", amount FROM fees')
    rows = source_cur.fetchall()
    
    for row in rows:
        target_cur.execute("""
            INSERT INTO fees (id, "desc", amount)
            VALUES (%s, %s, %s)
            ON CONFLICT (id) DO NOTHING
        """, row)
    
    target_conn.commit()
    print(f"  Migrated {len(rows)} fees")


def reset_sequences(conn):
    """Reset all sequences to max ID + 1."""
    print("Resetting sequences...")
    
    cur = conn.cursor()
    
    # Support both old (borrows*) and new (loans*) table names
    tables = [
        'users', 'items', 'specimens', 'authors', 
        'loans', 'borrows',
        'loans_settings', 'borrows_settings', 
        'loans_archives', 'borrows_archives', 
        'z3950servers', 'editions', 'collections', 'series', 
        'sources', 'account_types', 'fees', 'occupations'
    ]
    
    for table in tables:
        try:
            cur.execute(f"SELECT setval('{table}_id_seq', COALESCE((SELECT MAX(id) FROM {table}), 1))")
        except Exception:
            # Table or sequence might not exist
            conn.rollback()
    
    conn.commit()
    print("  Sequences reset")


def confirm_reset():
    """Ask for confirmation before resetting database."""
    print()
    print("=" * 60)
    print("WARNING: You are about to reset the target database!")
    print("This will DELETE ALL DATA in the target database.")
    print("=" * 60)
    print()
    response = input("Type 'yes' to confirm: ")
    return response.lower() == 'yes'


def main():
    parser = argparse.ArgumentParser(
        description='Migrate Elidune data from legacy database',
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  # Basic migration
  python migrate_data.py --source-db postgres://user:pass@host/old_db --target-db postgres://user:pass@host/new_db

  # Reset target and hash passwords (recommended for fresh migration)
  python migrate_data.py --source-db <old> --target-db <new> --reset

  # Skip specific tables
  python migrate_data.py --source-db <old> --target-db <new> --skip-items --skip-users
"""
    )
    parser.add_argument('--source-db', required=True, help='Source database URL')
    parser.add_argument('--target-db', required=True, help='Target database URL')
    parser.add_argument('--reset', action='store_true', 
                        help='Reset (truncate) all tables in target database before migration')

    parser.add_argument('--skip-items', action='store_true', help='Skip items migration')
    parser.add_argument('--skip-users', action='store_true', help='Skip users migration')
    parser.add_argument('--yes', '-y', action='store_true', help='Skip confirmation prompts')
    args = parser.parse_args()
    args.hash_passwords = True
    print()
    print("=" * 60)
    print("  Elidune Data Migration")
    print("=" * 60)
    print()
    print(f"Source: {args.source_db}")
    print(f"Target: {args.target_db}")
    print()
    print("Options:")
    print(f"  Reset target DB: {'Yes' if args.reset else 'No'}")
    print(f"  Hash passwords:  {'Yes' if args.hash_passwords else 'No'}")
    print(f"  Skip items:      {'Yes' if args.skip_items else 'No'}")
    print(f"  Skip users:      {'Yes' if args.skip_users else 'No'}")
    print()
    
    if args.hash_passwords and not ARGON2_AVAILABLE:
        print("ERROR: --hash-passwords requires argon2-cffi")
        print("Install with: pip install argon2-cffi")
        sys.exit(1)
    
    # Confirm reset if requested
    if args.reset and not args.yes:
        if not confirm_reset():
            print("Migration cancelled.")
            sys.exit(0)
    
    try:
        source_conn = connect_db(args.source_db)
        target_conn = connect_db(args.target_db)
        
        print("Connected to databases")
        print()
        
        # Detect target database schema (INTEGER or TIMESTAMPTZ for dates)
        use_timestamptz = is_timestamptz_schema(target_conn)
        if use_timestamptz:
            print("Detected TIMESTAMPTZ columns in target database")
            print("  -> Will convert Unix timestamps to TIMESTAMPTZ")
        else:
            print("Detected INTEGER columns in target database")
            print("  -> Will keep Unix timestamps as INTEGER")
        print()
        
        # Reset target database if requested
        if args.reset:
            if not reset_target_database(target_conn, args.target_db):
                print("ERROR: Failed to recreate database schema after reset")
                sys.exit(1)
        else:
            # If not resetting, ensure schema is up to date by running migrations
            print("Checking and applying schema migrations...")
            if not run_schema_migrations(target_conn):
                print("WARNING: Some migrations may have failed, but continuing with data migration...")
                print()
        
        # Migrate in order of dependencies
        migrate_account_types(source_conn, target_conn)
        
        if not args.skip_users:
            migrate_users(source_conn, target_conn, hash_passwords=args.hash_passwords, use_timestamptz=use_timestamptz)
        
        migrate_authors(source_conn, target_conn)
        migrate_editions(source_conn, target_conn)
        migrate_collections(source_conn, target_conn)
        migrate_series(source_conn, target_conn)
        migrate_sources(source_conn, target_conn)
        
        if not args.skip_items:
            migrate_items(source_conn, target_conn, use_timestamptz=use_timestamptz)
            migrate_specimens(source_conn, target_conn, use_timestamptz=use_timestamptz)
        
        migrate_loans(source_conn, target_conn, use_timestamptz=use_timestamptz)
        migrate_loans_archives(source_conn, target_conn, use_timestamptz=use_timestamptz)
        migrate_settings(source_conn, target_conn)
        migrate_z3950servers(source_conn, target_conn)
        migrate_fees(source_conn, target_conn)
        
        reset_sequences(target_conn)
        
        print()
        print("=" * 60)
        print("  Migration completed successfully!")
        print("=" * 60)
        
        if args.hash_passwords:
            print()
            print("Note: Passwords have been hashed with Argon2.")
            print("Users can now log in with their original passwords.")
        
    except Exception as e:
        print(f"Error: {e}")
        import traceback
        traceback.print_exc()
        sys.exit(1)
    finally:
        if 'source_conn' in locals():
            source_conn.close()
        if 'target_conn' in locals():
            target_conn.close()


if __name__ == '__main__':
    main()
