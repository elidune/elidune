-- MCP read-only query roles, grants, and row-level security.
-- Login role `elidune_mcp` has NO table grants; each query SET LOCAL ROLE to a
-- NOLOGIN role so forgotten SET ROLE cannot leak owner (BYPASSRLS) access.

-- ---------------------------------------------------------------------------
-- Roles (cluster-level; IF NOT EXISTS)
-- ---------------------------------------------------------------------------
DO $$
BEGIN
    IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'elidune_mcp_admin') THEN
        CREATE ROLE elidune_mcp_admin NOLOGIN NOSUPERUSER NOBYPASSRLS NOINHERIT;
    END IF;
    IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'elidune_mcp_staff') THEN
        CREATE ROLE elidune_mcp_staff NOLOGIN NOSUPERUSER NOBYPASSRLS NOINHERIT;
    END IF;
    IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'elidune_mcp_patron') THEN
        CREATE ROLE elidune_mcp_patron NOLOGIN NOSUPERUSER NOBYPASSRLS NOINHERIT;
    END IF;
    IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'elidune_mcp_catalog') THEN
        CREATE ROLE elidune_mcp_catalog NOLOGIN NOSUPERUSER NOBYPASSRLS NOINHERIT;
    END IF;
    IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'elidune_mcp') THEN
        CREATE ROLE elidune_mcp LOGIN PASSWORD 'elidune_mcp'
            NOSUPERUSER NOCREATEDB NOCREATEROLE NOINHERIT NOBYPASSRLS;
    END IF;
END
$$;

GRANT elidune_mcp_admin TO elidune_mcp WITH INHERIT FALSE;
GRANT elidune_mcp_staff TO elidune_mcp WITH INHERIT FALSE;
GRANT elidune_mcp_patron TO elidune_mcp WITH INHERIT FALSE;
GRANT elidune_mcp_catalog TO elidune_mcp WITH INHERIT FALSE;

DO $$
BEGIN
    EXECUTE format('GRANT CONNECT ON DATABASE %I TO elidune_mcp', current_database());
END
$$;

GRANT USAGE ON SCHEMA public TO elidune_mcp_admin, elidune_mcp_staff, elidune_mcp_patron, elidune_mcp_catalog;

-- ---------------------------------------------------------------------------
-- Safe views (secret columns never exposed). users is security_invoker so RLS applies.
-- ---------------------------------------------------------------------------
CREATE SCHEMA IF NOT EXISTS mcp;
GRANT USAGE ON SCHEMA mcp TO elidune_mcp_admin, elidune_mcp_staff, elidune_mcp_patron, elidune_mcp_catalog;

CREATE OR REPLACE VIEW mcp.users AS
SELECT
    id,
    login,
    firstname,
    lastname,
    email,
    addr_street,
    addr_zip_code,
    addr_city,
    phone,
    account_type,
    fee,
    group_id,
    barcode,
    notes,
    public_type,
    status,
    birthdate,
    created_at,
    update_at,
    expiry_at,
    archived_at,
    language,
    sex,
    staff_type,
    hours_per_week,
    staff_start_date,
    staff_end_date,
    receive_reminders,
    two_factor_enabled,
    two_factor_method,
    must_change_password
FROM public.users;

ALTER VIEW mcp.users SET (security_invoker = true);

COMMENT ON VIEW mcp.users IS 'MCP-safe users projection (no password, TOTP, recovery codes, token_version).';

CREATE OR REPLACE VIEW mcp.settings AS
SELECT
    key,
    CASE
        WHEN key = 'email' THEN COALESCE(value, '{}'::jsonb) - 'smtp_password'
        ELSE value
    END AS value,
    updated_at
FROM public.settings;

COMMENT ON VIEW mcp.settings IS 'Settings for MCP admin; smtp_password stripped from email JSON.';

CREATE OR REPLACE VIEW mcp.z3950servers AS
SELECT
    id,
    address,
    port,
    name,
    description,
    activated,
    login,
    database,
    format,
    encoding
FROM public.z3950servers;

COMMENT ON VIEW mcp.z3950servers IS 'Z39.50 servers without the password column.';

-- ---------------------------------------------------------------------------
-- Catalog (all MCP roles including guest)
-- ---------------------------------------------------------------------------
GRANT SELECT ON TABLE
    authors, editions, series, collections, sources,
    biblios, biblio_authors, biblio_series, biblio_collections,
    items, public_types, library_info, equipment, events,
    schedule_periods, schedule_slots, schedule_closures, fees
TO elidune_mcp_admin, elidune_mcp_staff, elidune_mcp_patron, elidune_mcp_catalog;

-- ---------------------------------------------------------------------------
-- Patron + staff + admin: identity-bearing circulation
-- ---------------------------------------------------------------------------
GRANT SELECT ON TABLE loans, holds, loans_archives
    TO elidune_mcp_admin, elidune_mcp_staff, elidune_mcp_patron;
GRANT SELECT ON mcp.users
    TO elidune_mcp_admin, elidune_mcp_staff, elidune_mcp_patron;

-- security_invoker views require SELECT on the underlying table. Column grants
-- hide secrets even if a client queries public.users explicitly.
GRANT SELECT (
    id, login, firstname, lastname, email,
    addr_street, addr_zip_code, addr_city, phone,
    account_type, fee, group_id, barcode, notes, public_type, status,
    birthdate, created_at, update_at, expiry_at, archived_at,
    language, sex, staff_type, hours_per_week, staff_start_date, staff_end_date,
    receive_reminders, two_factor_enabled, two_factor_method,
    must_change_password
) ON public.users TO elidune_mcp_admin, elidune_mcp_staff, elidune_mcp_patron;

-- ---------------------------------------------------------------------------
-- Staff + admin
-- ---------------------------------------------------------------------------
GRANT SELECT ON TABLE
    inventory_sessions, inventory_scans,
    loans_settings, public_type_loan_settings, account_types,
    saved_queries, visitor_counts
TO elidune_mcp_admin, elidune_mcp_staff;

-- ---------------------------------------------------------------------------
-- Admin only
-- ---------------------------------------------------------------------------
GRANT SELECT ON TABLE
    audit_log,
    email_outbox, email_outbox_reminder_loans, email_outbox_event_announcements,
    email_templates
TO elidune_mcp_admin;
GRANT SELECT ON mcp.settings, mcp.z3950servers TO elidune_mcp_admin;

-- Future tables created by the migration user (typically `elidune`) are readable by admin/staff.
ALTER DEFAULT PRIVILEGES IN SCHEMA public
    GRANT SELECT ON TABLES TO elidune_mcp_admin, elidune_mcp_staff;

-- ---------------------------------------------------------------------------
-- Row-level security (owner `elidune` bypasses unless FORCE; app REST unchanged)
-- ---------------------------------------------------------------------------
ALTER TABLE users ENABLE ROW LEVEL SECURITY;
ALTER TABLE loans ENABLE ROW LEVEL SECURITY;
ALTER TABLE holds ENABLE ROW LEVEL SECURITY;
ALTER TABLE loans_archives ENABLE ROW LEVEL SECURITY;
ALTER TABLE saved_queries ENABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS mcp_select ON users;
CREATE POLICY mcp_select ON users
    FOR SELECT
    USING (
        pg_has_role(current_user, 'elidune_mcp_admin', 'member')
        OR pg_has_role(current_user, 'elidune_mcp_staff', 'member')
        OR id = NULLIF(current_setting('elidune.current_user_id', true), '')::bigint
    );

DROP POLICY IF EXISTS mcp_select ON loans;
CREATE POLICY mcp_select ON loans
    FOR SELECT
    USING (
        pg_has_role(current_user, 'elidune_mcp_admin', 'member')
        OR pg_has_role(current_user, 'elidune_mcp_staff', 'member')
        OR user_id = NULLIF(current_setting('elidune.current_user_id', true), '')::bigint
    );

DROP POLICY IF EXISTS mcp_select ON holds;
CREATE POLICY mcp_select ON holds
    FOR SELECT
    USING (
        pg_has_role(current_user, 'elidune_mcp_admin', 'member')
        OR pg_has_role(current_user, 'elidune_mcp_staff', 'member')
        OR user_id = NULLIF(current_setting('elidune.current_user_id', true), '')::bigint
    );

DROP POLICY IF EXISTS mcp_select ON loans_archives;
CREATE POLICY mcp_select ON loans_archives
    FOR SELECT
    USING (
        pg_has_role(current_user, 'elidune_mcp_admin', 'member')
        OR pg_has_role(current_user, 'elidune_mcp_staff', 'member')
        OR user_id = NULLIF(current_setting('elidune.current_user_id', true), '')::bigint
    );

DROP POLICY IF EXISTS mcp_select ON saved_queries;
CREATE POLICY mcp_select ON saved_queries
    FOR SELECT
    USING (
        pg_has_role(current_user, 'elidune_mcp_admin', 'member')
        OR pg_has_role(current_user, 'elidune_mcp_staff', 'member')
        OR user_id = NULLIF(current_setting('elidune.current_user_id', true), '')::bigint
    );
