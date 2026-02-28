#!/usr/bin/env python3
"""
Data migration script for Elidune
Migrates data from the legacy C/XML-RPC PostgreSQL schema to the new Rust schema.

Source schema: legacy database (see elidune-pgdump.sql)
Target schema: new database (see init_database.sql)

Usage:
    python migrate_data.py --source-db <old_db_url> --target-db <new_db_url>
    python migrate_data.py --source-db <old_db_url> --target-db <new_db_url> --reset
    python migrate_data.py --source-db <old_db_url> --target-db <new_db_url> --skip-items --skip-users

Key transformations:
    - account_types: id (int) -> code (slug)
    - fees: id (int) -> code (slug), "desc" -> "name"
    - users: account_type_id -> account_type (slug), fee_id -> fee (slug),
             dates int -> timestamptz, passwords -> argon2 hash,
             removed: sex_id, subscription_type_id, occupation, profession
             added: status, language
    - items/specimens: dates int -> timestamptz, added lifecycle_status
    - borrows -> loans (returned -> loans_archives)
    - borrows_archives -> loans_archives (account_type_id -> account_type slug)
    - borrows_settings -> loans_settings (account_type_id -> account_type slug)
    - z3950servers: added encoding (default utf-8)

Requirements:
    pip install psycopg2-binary argon2-cffi
"""

import argparse
import re
import sys
from datetime import datetime, timezone
from pathlib import Path

import psycopg2

try:
    from argon2 import PasswordHasher
    ARGON2_AVAILABLE = True
except ImportError:
    ARGON2_AVAILABLE = False
    print("Warning: argon2-cffi not installed. Install with: pip install argon2-cffi")


# =============================================================================
# CONSTANTS - Source to target mappings
# =============================================================================

ACCOUNT_TYPE_ID_TO_CODE = {
    1: 'guest',
    2: 'reader',
    3: 'librarian',
    4: 'admin',
    8: 'group',
}

FEE_ID_TO_CODE = {
    1: 'free',
    2: 'local',
    3: 'foreigner',
}

TABLES_DROP_ORDER = [
    'loans_archives', 'loans', 'loans_settings',
    'specimens', 'remote_specimens',
    'items', 'remote_items',
    'z3950servers', 'fees', 'users',
    'authors', 'editions', 'collections', 'series', 'sources',
    'account_types',
]


# =============================================================================
# HELPERS
# =============================================================================

def connect_db(url):
    """Connect to PostgreSQL database."""
    return psycopg2.connect(url)


def ts_to_datetime(value):
    """Convert a Unix timestamp (int) to a UTC datetime, or None."""
    if value is None or value == 0:
        return None
    if isinstance(value, datetime):
        return value
    try:
        numeric = int(value) if isinstance(value, str) else float(value)
        return datetime.fromtimestamp(numeric, tz=timezone.utc)
    except (ValueError, TypeError, OSError, OverflowError):
        return None


def hash_password(plain, hasher):
    """Hash a password with Argon2. Returns None for empty passwords."""
    if not plain:
        return None
    if plain.startswith('$argon2'):
        return plain  # already hashed
    return hasher.hash(plain)


def slug_from_name(name, fallback_id):
    """Generate a slug from a name string."""
    if not name:
        return f'item_{fallback_id}'
    slug = re.sub(r'[^a-z0-9]+', '_', name.lower().strip()).strip('_')
    return slug or f'item_{fallback_id}'


def batch_insert(cursor, conn, sql, rows, batch_size=500):
    """Execute INSERT statements in batches and commit."""
    for i in range(0, len(rows), batch_size):
        for row in rows[i:i + batch_size]:
            cursor.execute(sql, row)
        conn.commit()


# =============================================================================
# SCHEMA MANAGEMENT
# =============================================================================

def reset_target_database(conn):
    """Drop all tables and recreate schema from init_database.sql."""
    print("Resetting target database...")
    cur = conn.cursor()

    for table in TABLES_DROP_ORDER:
        cur.execute(f"DROP TABLE IF EXISTS {table} CASCADE")

    cur.execute("DROP FUNCTION IF EXISTS items_search_vector_update() CASCADE")
    conn.commit()
    print("  Tables dropped")

    # Run init_database.sql
    init_sql_path = Path(__file__).parent / 'init_database.sql'
    if not init_sql_path.exists():
        print(f"  ERROR: {init_sql_path} not found")
        sys.exit(1)

    sql = init_sql_path.read_text(encoding='utf-8')
    cur.execute(sql)
    conn.commit()
    print("  Schema recreated from init_database.sql")


# =============================================================================
# DATA MIGRATION FUNCTIONS
# =============================================================================

def migrate_account_types(src, dst):
    """Migrate account_types: id (int PK) -> code (slug PK)."""
    print("Migrating account_types...")
    src_cur = src.cursor()
    dst_cur = dst.cursor()

    src_cur.execute("""
        SELECT id, name, items_rights, users_rights, loans_rights,
               items_archive_rights, borrows_rights, settings_rights
        FROM account_types
    """)

    count = 0
    for row in src_cur.fetchall():
        aid, name, ir, ur, lr, iar, br, sr = row
        code = ACCOUNT_TYPE_ID_TO_CODE.get(aid, f'type_{aid}')
        dst_cur.execute("""
            INSERT INTO account_types (code, name, items_rights, users_rights, loans_rights,
                                       items_archive_rights, borrows_rights, settings_rights)
            VALUES (%s, %s, %s, %s, %s, %s, %s, %s)
            ON CONFLICT (code) DO UPDATE SET
                name = EXCLUDED.name, items_rights = EXCLUDED.items_rights,
                users_rights = EXCLUDED.users_rights, loans_rights = EXCLUDED.loans_rights,
                items_archive_rights = EXCLUDED.items_archive_rights,
                borrows_rights = EXCLUDED.borrows_rights, settings_rights = EXCLUDED.settings_rights
        """, (code, name, ir, ur, lr, iar, br, sr))
        count += 1

    dst.commit()
    print(f"  {count} account types migrated")


def migrate_fees(src, dst):
    """Migrate fees: id (int PK) + desc -> code (slug PK) + name."""
    print("Migrating fees...")
    src_cur = src.cursor()
    dst_cur = dst.cursor()

    # Source uses "desc" column, not "name"
    src_cur.execute('SELECT id, "desc", amount FROM fees')

    count = 0
    for fee_id, desc, amount in src_cur.fetchall():
        code = FEE_ID_TO_CODE.get(fee_id, slug_from_name(desc, fee_id))
        dst_cur.execute("""
            INSERT INTO fees (code, name, amount)
            VALUES (%s, %s, %s)
            ON CONFLICT (code) DO UPDATE SET name = EXCLUDED.name, amount = EXCLUDED.amount
        """, (code, desc, amount))
        count += 1

    dst.commit()
    print(f"  {count} fees migrated")


def migrate_users(src, dst, hash_passwords=True):
    """Migrate users with password hashing and schema transformations.

    Source columns: id, login, password, firstname, lastname, email,
        addr_street, addr_zip_code, addr_city, phone, sex_id (dropped),
        account_type_id -> account_type (slug), subscription_type_id (dropped),
        fee_id -> fee (slug), last_payement_date (dropped), group_id, barcode,
        notes, occupation (dropped), crea_date (int), modif_date (int),
        issue_date (int), profession (dropped), birthdate, archived_date (int),
        public_type

    Target adds: status, language, 2FA fields (defaults)
    """
    print("Migrating users...")

    hasher = None
    if hash_passwords:
        if not ARGON2_AVAILABLE:
            print("  ERROR: argon2-cffi required. Install with: pip install argon2-cffi")
            sys.exit(1)
        hasher = PasswordHasher()
        print("  Password hashing enabled (Argon2)")

    src_cur = src.cursor()
    dst_cur = dst.cursor()

    src_cur.execute("""
        SELECT id, login, password, firstname, lastname, email,
               addr_street, addr_zip_code, addr_city, phone,
               account_type_id, fee_id, group_id, barcode,
               notes, crea_date, modif_date, issue_date,
               birthdate, archived_date, public_type
        FROM users
    """)
    rows = src_cur.fetchall()

    # Build unique login map
    login_counts = {}
    for row in rows:
        login = (row[1] or '').strip() or f'user_{row[0]}'
        key = login.lower()
        login_counts.setdefault(key, []).append((row[0], login))

    unique_logins = {}
    for key, entries in login_counts.items():
        if len(entries) == 1:
            unique_logins[entries[0][0]] = entries[0][1]
        else:
            for uid, login in entries:
                unique_logins[uid] = f'{login}_{uid}'

    hashed_count = 0
    migrated = 0

    for row in rows:
        (uid, _, raw_pw, firstname, lastname, email,
         addr_street, addr_zip_code, addr_city, phone,
         account_type_id, fee_id, group_id, barcode,
         notes, crea_date, modif_date, issue_date,
         birthdate, archived_date, public_type) = row

        login = unique_logins.get(uid, f'user_{uid}')
        email = email.strip() if email and email.strip() else None

        # Password hashing
        password = None
        if hash_passwords and raw_pw:
            password = hash_password(raw_pw, hasher)
            if password != raw_pw:
                hashed_count += 1

        # Convert IDs to codes
        account_type = ACCOUNT_TYPE_ID_TO_CODE.get(account_type_id, 'guest')
        fee = FEE_ID_TO_CODE.get(fee_id) if fee_id else None

        # Convert dates
        crea_dt = ts_to_datetime(crea_date)
        modif_dt = ts_to_datetime(modif_date)
        issue_dt = ts_to_datetime(issue_date)
        archived_dt = ts_to_datetime(archived_date)

        # Compute status: 0=active, 2=archived/deleted
        status = 2 if archived_dt else 0

        dst_cur.execute("""
            INSERT INTO users (
                id, login, password, firstname, lastname, email,
                addr_street, addr_zip_code, addr_city, phone,
                account_type, fee, group_id, barcode, notes,
                public_type, status, birthdate,
                crea_date, modif_date, issue_date, archived_date,
                language
            ) VALUES (
                %s, %s, %s, %s, %s, %s,
                %s, %s, %s, %s,
                %s, %s, %s, %s, %s,
                %s, %s, %s,
                %s, %s, %s, %s,
                'fr'
            ) ON CONFLICT (id) DO UPDATE SET
                login = EXCLUDED.login,
                password = EXCLUDED.password,
                firstname = EXCLUDED.firstname,
                lastname = EXCLUDED.lastname
        """, (
            uid, login, password, firstname, lastname, email,
            addr_street, addr_zip_code, addr_city, phone,
            account_type, fee, group_id, barcode, notes,
            public_type, status, birthdate,
            crea_dt, modif_dt, issue_dt, archived_dt,
        ))
        migrated += 1

    dst.commit()
    parts = [f"{migrated} users migrated"]
    if hash_passwords:
        parts.append(f"{hashed_count} passwords hashed")
    print(f"  {', '.join(parts)}")


def migrate_simple_table(src, dst, table, columns, conflict_col="id"):
    """Migrate a table with identical source/target structure."""
    print(f"Migrating {table}...")
    src_cur = src.cursor()
    dst_cur = dst.cursor()

    cols_str = ", ".join(columns)
    placeholders = ", ".join(["%s"] * len(columns))

    src_cur.execute(f"SELECT {cols_str} FROM {table}")
    rows = src_cur.fetchall()

    for row in rows:
        dst_cur.execute(f"""
            INSERT INTO {table} ({cols_str})
            VALUES ({placeholders})
            ON CONFLICT ({conflict_col}) DO NOTHING
        """, row)

    dst.commit()
    print(f"  {len(rows)} {table} migrated")


def migrate_items(src, dst):
    """Migrate items: dates int->timestamptz, add lifecycle_status/archived_date."""
    print("Migrating items...")
    src_cur = src.cursor()
    dst_cur = dst.cursor()

    src_cur.execute("SELECT COUNT(*) FROM items")
    total = src_cur.fetchone()[0]

    BATCH = 1000
    offset = 0

    while offset < total:
        src_cur.execute(f"""
            SELECT id, media_type, identification, price, barcode, dewey,
                   publication_date, lang, lang_orig, title1, title2, title3, title4,
                   author1_ids, author1_functions, author2_ids, author2_functions,
                   author3_ids, author3_functions, serie_id, serie_vol_number,
                   collection_id, collection_number_sub, collection_vol_number,
                   source_id, genre, subject, public_type,
                   edition_id, edition_date, nb_pages, format, content, addon,
                   abstract, notes, keywords, nb_specimens, state,
                   is_archive, archived_timestamp, is_valid, crea_date, modif_date
            FROM items ORDER BY id LIMIT {BATCH} OFFSET {offset}
        """)

        for row in src_cur.fetchall():
            vals = list(row)
            is_archive = vals[39]

            # Convert dates (indices: archived_timestamp=42, crea_date=44, modif_date=45)
            archived_ts = ts_to_datetime(vals[40])
            crea_dt = ts_to_datetime(vals[42])
            modif_dt = ts_to_datetime(vals[43])
            vals[40] = archived_ts
            vals[42] = crea_dt
            vals[43] = modif_dt

            lifecycle_status = 2 if is_archive == 1 else 0
            archived_date = archived_ts if is_archive == 1 else None

            dst_cur.execute("""
                INSERT INTO items (
                    id, media_type, identification, price, barcode, dewey,
                    publication_date, lang, lang_orig, title1, title2, title3, title4,
                    author1_ids, author1_functions, author2_ids, author2_functions,
                    author3_ids, author3_functions, serie_id, serie_vol_number,
                    collection_id, collection_number_sub, collection_vol_number,
                    source_id, genre, subject, public_type,
                    edition_id, edition_date, nb_pages, format, content, addon,
                    abstract, notes, keywords, nb_specimens, state,
                    is_archive, archived_timestamp, is_valid, crea_date, modif_date,
                    lifecycle_status, archived_date
                ) VALUES (
                    %s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,
                    %s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,
                    %s,%s,%s,%s, %s,%s
                ) ON CONFLICT (id) DO NOTHING
            """, vals + [lifecycle_status, archived_date])

        dst.commit()
        offset += BATCH
        print(f"  {min(offset, total)}/{total} items")

    print(f"  {total} items migrated")


def migrate_remote_items(src, dst):
    """Migrate remote_items: dates int->timestamptz, add lifecycle_status."""
    print("Migrating remote_items...")
    src_cur = src.cursor()
    dst_cur = dst.cursor()

    src_cur.execute("""
        SELECT id, media_type, identification, price, barcode, dewey,
               publication_date, lang, lang_orig, title1, title2, title3, title4,
               author1_ids, author1_functions, author2_ids, author2_functions,
               author3_ids, author3_functions, serie_id, serie_vol_number,
               collection_id, collection_number_sub, collection_vol_number,
               source_id, genre, subject, public_type,
               edition_id, edition_date, nb_pages, format, content, addon,
               abstract, notes, keywords, nb_specimens, state,
               is_archive, archived_timestamp, is_valid, crea_date, modif_date
        FROM remote_items
    """)
    rows = src_cur.fetchall()

    for row in rows:
        vals = list(row)
        is_archive = vals[41]
        vals[40] = ts_to_datetime(vals[40])  # archived_timestamp
        vals[42] = ts_to_datetime(vals[42])  # crea_date
        vals[43] = ts_to_datetime(vals[43])  # modif_date
        lifecycle_status = 2 if is_archive == 1 else 0

        dst_cur.execute("""
            INSERT INTO remote_items (
                id, media_type, identification, price, barcode, dewey,
                publication_date, lang, lang_orig, title1, title2, title3, title4,
                author1_ids, author1_functions, author2_ids, author2_functions,
                author3_ids, author3_functions, serie_id, serie_vol_number,
                collection_id, collection_number_sub, collection_vol_number,
                source_id, genre, subject, public_type,
                edition_id, edition_date, nb_pages, format, content, addon,
                abstract, notes, keywords, nb_specimens, state,
                is_archive, archived_timestamp, is_valid, crea_date, modif_date,
                lifecycle_status
            ) VALUES (
                %s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,
                %s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,
                %s,%s,%s,%s, %s
            ) ON CONFLICT (id) DO NOTHING
        """, vals + [lifecycle_status])

    dst.commit()
    print(f"  {len(rows)} remote_items migrated")


def migrate_specimens(src, dst):
    """Migrate specimens: dates int->timestamptz, add lifecycle_status."""
    print("Migrating specimens...")
    src_cur = src.cursor()
    dst_cur = dst.cursor()

    src_cur.execute("SELECT COUNT(*) FROM specimens")
    total = src_cur.fetchone()[0]

    BATCH = 1000
    offset = 0

    while offset < total:
        src_cur.execute(f"""
            SELECT id, id_item, source_id, identification, cote, place,
                   status, codestat, notes, price, modif_date,
                   is_archive, archive_date, crea_date
            FROM specimens ORDER BY id LIMIT {BATCH} OFFSET {offset}
        """)

        for row in src_cur.fetchall():
            vals = list(row)
            is_archive = vals[11]
            vals[10] = ts_to_datetime(vals[10])  # modif_date
            vals[12] = ts_to_datetime(vals[12])  # archive_date
            vals[13] = ts_to_datetime(vals[13])  # crea_date
            lifecycle_status = 2 if is_archive == 1 else 0

            dst_cur.execute("""
                INSERT INTO specimens (
                    id, id_item, source_id, identification, cote, place,
                    status, codestat, notes, price, modif_date,
                    is_archive, archive_date, crea_date, lifecycle_status
                ) VALUES (%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s)
                ON CONFLICT (id) DO NOTHING
            """, vals + [lifecycle_status])

        dst.commit()
        offset += BATCH
        print(f"  {min(offset, total)}/{total} specimens")

    print(f"  {total} specimens migrated")


def migrate_remote_specimens(src, dst):
    """Migrate remote_specimens: dates int->timestamptz, add lifecycle_status."""
    print("Migrating remote_specimens...")
    src_cur = src.cursor()
    dst_cur = dst.cursor()

    src_cur.execute("""
        SELECT id, id_item, source_id, identification, cote, media_type,
               place, status, codestat, notes, price, creation_date, modif_date
        FROM remote_specimens
    """)
    rows = src_cur.fetchall()

    for row in rows:
        vals = list(row)
        vals[11] = ts_to_datetime(vals[11])  # creation_date
        vals[12] = ts_to_datetime(vals[12])  # modif_date

        dst_cur.execute("""
            INSERT INTO remote_specimens (
                id, id_item, source_id, identification, cote, media_type,
                place, status, codestat, notes, price, creation_date, modif_date,
                lifecycle_status
            ) VALUES (%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s, 0)
            ON CONFLICT (id) DO NOTHING
        """, vals)

    dst.commit()
    print(f"  {len(rows)} remote_specimens migrated")


def migrate_loans(src, dst):
    """Migrate borrows -> loans + loans_archives.

    Active loans (returned_date IS NULL or 0) -> loans table
    Returned loans (returned_date set) -> loans_archives with user enrichment
    """
    print("Migrating borrows -> loans/loans_archives...")
    src_cur = src.cursor()
    dst_cur = dst.cursor()

    # Load user info for archive enrichment
    src_cur.execute("SELECT id, addr_city, account_type_id, public_type FROM users")
    user_info = {}
    for uid, city, at_id, pt in src_cur.fetchall():
        at_code = ACCOUNT_TYPE_ID_TO_CODE.get(at_id, 'guest') if at_id else 'guest'
        user_info[uid] = (city, at_code, pt)

    # Load specimen -> item_id mapping
    src_cur.execute("SELECT id, id_item FROM specimens")
    specimen_to_item = {r[0]: r[1] for r in src_cur.fetchall()}

    src_cur.execute("""
        SELECT id, user_id, specimen_id, item_id, date, renew_date,
               nb_renews, issue_date, notes, returned_date
        FROM borrows
    """)
    borrows = src_cur.fetchall()

    active = 0
    archived = 0
    skipped = 0

    for row in borrows:
        vals = list(row)
        user_id = vals[1]
        specimen_id = vals[2]
        returned_date_raw = vals[9]

        # Convert dates
        vals[4] = ts_to_datetime(vals[4])   # date
        vals[5] = ts_to_datetime(vals[5])   # renew_date
        vals[7] = ts_to_datetime(vals[7])   # issue_date
        vals[9] = ts_to_datetime(vals[9])   # returned_date

        if returned_date_raw and returned_date_raw != 0:
            # Returned loan -> loans_archives (item_id removed; link via specimen_id only)
            if specimen_id is None:
                skipped += 1
                continue

            city, at_code, pt = user_info.get(user_id, (None, 'guest', None))

            dst_cur.execute("""
                INSERT INTO loans_archives (
                    id, user_id, specimen_id, date, nb_renews,
                    issue_date, returned_date, notes,
                    borrower_public_type, addr_city, account_type
                ) VALUES (%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s)
                ON CONFLICT (id) DO NOTHING
            """, (
                vals[0], user_id, specimen_id,
                vals[4], vals[6], vals[7], vals[9], vals[8],
                pt, city, at_code,
            ))
            archived += 1
        else:
            # Active loan -> loans (item_id removed; link via specimen_id only)
            dst_cur.execute("""
                INSERT INTO loans (
                    id, user_id, specimen_id, date, renew_date,
                    nb_renews, issue_date, notes, returned_date
                ) VALUES (%s,%s,%s,%s,%s,%s,%s,%s,%s)
                ON CONFLICT (id) DO NOTHING
            """, (vals[0], vals[1], vals[2], vals[4], vals[5], vals[6], vals[7], vals[8], vals[9]))
            active += 1

    dst.commit()
    print(f"  {len(borrows)} borrows processed: {active} active loans, {archived} archived")
    if skipped:
        print(f"  {skipped} skipped (missing specimen_id)")


def migrate_loans_archives(src, dst):
    """Migrate borrows_archives -> loans_archives.

    Source has: sex_id (dropped), account_type_id -> account_type (slug), no user_id
    """
    print("Migrating borrows_archives -> loans_archives...")
    src_cur = src.cursor()
    dst_cur = dst.cursor()

    src_cur.execute("""
        SELECT id, item_id, specimen_id, date, nb_renews, issue_date,
               returned_date, notes, borrower_public_type,
               addr_city, account_type_id
        FROM borrows_archives
    """)
    rows = src_cur.fetchall()

    migrated = 0
    skipped = 0

    for row in rows:
        vals = list(row)
        specimen_id = vals[2]
        at_id = vals[10]

        if specimen_id is None:
            skipped += 1
            continue

        # Convert dates
        date_val = ts_to_datetime(vals[3])
        issue_dt = ts_to_datetime(vals[5])
        returned_dt = ts_to_datetime(vals[6])

        at_code = ACCOUNT_TYPE_ID_TO_CODE.get(at_id, 'guest') if at_id else 'guest'

        dst_cur.execute("""
            INSERT INTO loans_archives (
                id, specimen_id, date, nb_renews, issue_date,
                returned_date, notes, borrower_public_type,
                addr_city, account_type
            ) VALUES (%s,%s,%s,%s,%s,%s,%s,%s,%s,%s)
            ON CONFLICT (id) DO NOTHING
        """, (
            vals[0], specimen_id, date_val, vals[4], issue_dt,
            returned_dt, vals[7], vals[8], vals[9], at_code,
        ))
        migrated += 1

    dst.commit()
    print(f"  {migrated}/{len(rows)} loans_archives migrated")
    if skipped:
        print(f"  {skipped} skipped (missing specimen_id)")


def migrate_loans_settings(src, dst):
    """Migrate borrows_settings -> loans_settings: account_type_id -> account_type slug."""
    print("Migrating borrows_settings -> loans_settings...")
    src_cur = src.cursor()
    dst_cur = dst.cursor()

    src_cur.execute("""
        SELECT id, media_type, nb_max, nb_renews, duration, notes, account_type_id
        FROM borrows_settings
    """)
    rows = src_cur.fetchall()

    for row in rows:
        sid, media_type, nb_max, nb_renews, duration, notes, at_id = row
        at_code = ACCOUNT_TYPE_ID_TO_CODE.get(at_id) if at_id else None

        dst_cur.execute("""
            INSERT INTO loans_settings (id, media_type, nb_max, nb_renews, duration, notes, account_type)
            VALUES (%s, %s, %s, %s, %s, %s, %s)
            ON CONFLICT (media_type) DO UPDATE SET
                nb_max = EXCLUDED.nb_max, nb_renews = EXCLUDED.nb_renews,
                duration = EXCLUDED.duration, notes = EXCLUDED.notes,
                account_type = EXCLUDED.account_type
        """, (sid, media_type, nb_max, nb_renews, duration, notes, at_code))

    dst.commit()
    print(f"  {len(rows)} loans_settings migrated")


def migrate_z3950servers(src, dst):
    """Migrate z3950servers: add encoding (default utf-8)."""
    print("Migrating z3950servers...")
    src_cur = src.cursor()
    dst_cur = dst.cursor()

    src_cur.execute("""
        SELECT id, address, port, name, description, activated,
               login, password, database, format
        FROM z3950servers
    """)
    rows = src_cur.fetchall()

    for row in rows:
        dst_cur.execute("""
            INSERT INTO z3950servers (
                id, address, port, name, description, activated,
                login, password, database, format, encoding
            ) VALUES (%s,%s,%s,%s,%s,%s,%s,%s,%s,%s, 'utf-8')
            ON CONFLICT (id) DO NOTHING
        """, row)

    dst.commit()
    print(f"  {len(rows)} z3950servers migrated")


def reset_sequences(conn):
    """Reset all sequences to their max ID value."""
    print("Resetting sequences...")
    cur = conn.cursor()

    tables = [
        'users', 'authors', 'editions', 'collections', 'series', 'sources',
        'items', 'remote_items', 'specimens', 'remote_specimens',
        'loans', 'loans_archives', 'loans_settings', 'z3950servers',
    ]

    for table in tables:
        try:
            cur.execute(f"SELECT setval('{table}_id_seq', COALESCE((SELECT MAX(id) FROM {table}), 1), true)")
        except Exception:
            conn.rollback()

    conn.commit()
    print("  Sequences reset")


# =============================================================================
# MAIN
# =============================================================================

def main():
    parser = argparse.ArgumentParser(
        description='Migrate Elidune data from legacy to new schema',
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  # Full migration with reset (recommended for fresh target)
  python migrate_data.py --source-db postgres://user:pass@host/old --target-db postgres://user:pass@host/new --reset -y

  # Incremental migration (target schema must already exist)
  python migrate_data.py --source-db <old> --target-db <new>

  # Skip specific tables
  python migrate_data.py --source-db <old> --target-db <new> --skip-items --skip-users
""",
    )
    parser.add_argument('--source-db', required=True, help='Source database URL')
    parser.add_argument('--target-db', required=True, help='Target database URL')
    parser.add_argument('--reset', action='store_true', help='Drop and recreate target schema before migration')
    parser.add_argument('--skip-items', action='store_true', help='Skip items/specimens migration')
    parser.add_argument('--skip-users', action='store_true', help='Skip users migration')
    parser.add_argument('--no-hash', action='store_true', help='Skip password hashing (migrate plaintext)')
    parser.add_argument('-y', '--yes', action='store_true', help='Skip confirmation prompts')
    args = parser.parse_args()

    hash_passwords = not args.no_hash

    print()
    print("=" * 60)
    print("  Elidune Data Migration")
    print("=" * 60)
    print()
    print(f"Source: {args.source_db}")
    print(f"Target: {args.target_db}")
    print(f"Options: reset={args.reset}, hash={hash_passwords}, "
          f"skip_items={args.skip_items}, skip_users={args.skip_users}")
    print()

    if hash_passwords and not ARGON2_AVAILABLE:
        print("ERROR: argon2-cffi required for password hashing.")
        print("Install with: pip install argon2-cffi")
        print("Or use --no-hash to skip password hashing.")
        sys.exit(1)

    if args.reset and not args.yes:
        print("WARNING: --reset will DELETE ALL DATA in the target database.")
        if input("Type 'yes' to confirm: ").strip().lower() != 'yes':
            print("Cancelled.")
            sys.exit(0)

    try:
        src = connect_db(args.source_db)
        dst = connect_db(args.target_db)
        print("Connected to databases")
        print()

        if args.reset:
            reset_target_database(dst)
            print()

        # Migrate in dependency order
        migrate_account_types(src, dst)
        migrate_fees(src, dst)

        if not args.skip_users:
            migrate_users(src, dst, hash_passwords=hash_passwords)

        migrate_simple_table(src, dst, 'authors', ['id', 'key', 'lastname', 'firstname', 'bio', 'notes'])
        migrate_simple_table(src, dst, 'editions', ['id', 'key', 'name', 'place', 'notes'])
        migrate_simple_table(src, dst, 'collections', ['id', 'key', 'title1', 'title2', 'title3', 'issn'])
        migrate_simple_table(src, dst, 'series', ['id', 'key', 'name'])
        migrate_simple_table(src, dst, 'sources', ['id', 'key', 'name'])

        if not args.skip_items:
            migrate_items(src, dst)
            migrate_remote_items(src, dst)
            migrate_specimens(src, dst)
            migrate_remote_specimens(src, dst)

        migrate_loans(src, dst)
        migrate_loans_archives(src, dst)
        migrate_loans_settings(src, dst)
        migrate_z3950servers(src, dst)

        reset_sequences(dst)

        print()
        print("=" * 60)
        print("  Migration completed successfully!")
        print("=" * 60)
        if hash_passwords:
            print("  Passwords hashed with Argon2 - users can log in with original passwords.")

    except Exception as e:
        print(f"\nERROR: {e}")
        import traceback
        traceback.print_exc()
        sys.exit(1)
    finally:
        for conn_name in ('src', 'dst'):
            if conn_name in locals():
                locals()[conn_name].close()


if __name__ == '__main__':
    main()
