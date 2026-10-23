-- =============================================================================
-- Elidune — SQLx migration 001 (initial schema)
-- =============================================================================
-- Canonical schema for fresh databases (squashed through former migration 072).
-- Apply with: sqlx migrate run
-- Linked as database/init_database.sql for tooling and docs.
--
-- Existing databases already at former 072: run once before deploying this build:
--   psql "$DATABASE_URL" -f database/reset_sqlx_migrations.sql
-- =============================================================================

--
--

--
-- Name: unaccent; Type: EXTENSION; Schema: -; Owner: -
--

CREATE EXTENSION IF NOT EXISTS unaccent;

--
-- Name: EXTENSION unaccent; Type: COMMENT; Schema: -; Owner: -
--

COMMENT ON EXTENSION unaccent IS 'text search dictionary that removes accents';

--
-- Name: account_types; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE account_types (
    code character varying(50) NOT NULL,
    name character varying(100),
    items_rights character varying(1),
    users_rights character varying(1),
    loans_rights character varying(1),
    items_archive_rights character varying(1),
    holds_rights character varying(1),
    settings_rights character varying(1),
    events_rights character varying(1),
    acquisitions_rights character varying(1) DEFAULT 'n'::character varying,
    weeding_rights character varying(1) DEFAULT 'n'::character varying NOT NULL,
    weeding_cancel_rights character varying(1) DEFAULT 'n'::character varying NOT NULL,
    is_system boolean DEFAULT false NOT NULL,
    circulation_override_rights character varying(1) DEFAULT 'n'::character varying NOT NULL
);

--
-- Name: COLUMN account_types.items_archive_rights; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN account_types.items_archive_rights IS 'n/r/w: delete a physical copy (DELETE /items/{id}). Distinct from weeding_rights.';

--
-- Name: COLUMN account_types.holds_rights; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN account_types.holds_rights IS 'n (none), o (own holds: self-service only), r (read queues/lists), w (full circulation + holds management)';

--
-- Name: COLUMN account_types.events_rights; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN account_types.events_rights IS 'n/r/w: none, read, or write access to /events';

--
-- Name: COLUMN account_types.acquisitions_rights; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN account_types.acquisitions_rights IS 'n/r/w: none, read, or write access to /acquisitions (distinct from items_rights / cataloging)';

--
-- Name: COLUMN account_types.weeding_rights; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN account_types.weeding_rights IS 'n/r/w: withdraw copies, record disposal, complete legacy rows, list and export. Distinct from items_rights.';

--
-- Name: COLUMN account_types.weeding_cancel_rights; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN account_types.weeding_cancel_rights IS 'n/r/w: cancel an active withdrawal that is not yet disposed. Write is required. Distinct from weeding_rights.';

--
-- Name: COLUMN account_types.is_system; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN account_types.is_system IS 'Built-in account types cannot be deleted. Custom types are false.';

--
-- Name: COLUMN account_types.circulation_override_rights; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN account_types.circulation_override_rights IS 'n/r/w. Write may force a loan past a hold for another patron and may force checkout or renewal past subscription_expired. A lost card and an open loan stay non-forceable. A reason is required. Distinct from settings_rights.';

--
-- Name: acquisition_funds; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE acquisition_funds (
    id bigint NOT NULL,
    code character varying(50) NOT NULL,
    name character varying(200) NOT NULL,
    fiscal_year integer NOT NULL,
    allocated_amount numeric(14,2) DEFAULT 0 NOT NULL,
    currency character varying(3) DEFAULT 'EUR'::character varying NOT NULL,
    notes text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);

--
-- Name: audit_log; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE audit_log (
    id bigint NOT NULL,
    event_type text NOT NULL,
    user_id bigint,
    entity_type text,
    entity_id bigint,
    ip_address text,
    payload jsonb,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    outcome text DEFAULT 'success'::text NOT NULL,
    http_status smallint,
    error_code text,
    error_message text,
    CONSTRAINT audit_log_outcome_check CHECK ((outcome = ANY (ARRAY['success'::text, 'failure'::text])))
);

--
-- Name: audit_log_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE audit_log_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

--
-- Name: audit_log_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE audit_log_id_seq OWNED BY audit_log.id;

--
-- Name: authors; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE authors (
    id bigint NOT NULL,
    key character varying(255),
    lastname character varying(255),
    firstname character varying(255),
    bio text,
    notes text,
    created_at timestamp with time zone DEFAULT now(),
    update_at timestamp with time zone
);

--
-- Name: authors_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE authors_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

--
-- Name: authors_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE authors_id_seq OWNED BY authors.id;

--
-- Name: biblio_authors; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE biblio_authors (
    id bigint NOT NULL,
    biblio_id bigint NOT NULL,
    author_id bigint NOT NULL,
    function character varying(50),
    author_type smallint DEFAULT 0 NOT NULL,
    "position" smallint DEFAULT 1 NOT NULL
);

--
-- Name: biblio_authors_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE biblio_authors_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

--
-- Name: biblio_authors_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE biblio_authors_id_seq OWNED BY biblio_authors.id;

--
-- Name: biblio_collections; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE biblio_collections (
    id bigint NOT NULL,
    biblio_id bigint NOT NULL,
    collection_id bigint NOT NULL,
    "position" smallint DEFAULT 1 NOT NULL,
    volume_number smallint
);

--
-- Name: biblio_collections_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE biblio_collections_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

--
-- Name: biblio_collections_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE biblio_collections_id_seq OWNED BY biblio_collections.id;

--
-- Name: biblio_series; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE biblio_series (
    id bigint NOT NULL,
    biblio_id bigint NOT NULL,
    series_id bigint NOT NULL,
    "position" smallint DEFAULT 1 NOT NULL,
    volume_number smallint
);

--
-- Name: biblio_series_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE biblio_series_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

--
-- Name: biblio_series_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE biblio_series_id_seq OWNED BY biblio_series.id;

--
-- Name: biblios; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE biblios (
    id bigint NOT NULL,
    media_type character varying(30) DEFAULT 'unknown'::character varying NOT NULL,
    isbn character varying(30),
    title character varying(500),
    subject text,
    audience_type character varying(30),
    lang character varying(32),
    lang_orig character varying(32),
    publication_date character varying(20),
    site_id bigint,
    edition_id bigint,
    page_extent text,
    format text,
    table_of_contents text,
    accompanying_material text,
    abstract text,
    notes text,
    keywords character varying[],
    is_valid boolean DEFAULT true NOT NULL,
    marc_record jsonb,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    archived_at timestamp with time zone,
    dewey character varying(100),
    genre smallint
);

--
-- Name: COLUMN biblios.site_id; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN biblios.site_id IS 'Legacy column — not used by the application. Site is stored on items.site_id per copy.';

--
-- Name: COLUMN biblios.genre; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN biblios.genre IS 'Legacy genre classification (SMALLINT codes matching Genre enum). NULL/0 = unknown.';

--
-- Name: biblios_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE biblios_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

--
-- Name: biblios_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE biblios_id_seq OWNED BY biblios.id;

--
-- Name: circulation_settings; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE circulation_settings (
    id smallint DEFAULT 1 NOT NULL,
    unpaid_fine_threshold bigint DEFAULT 0 NOT NULL,
    max_active_holds smallint DEFAULT 20 NOT NULL,
    pick_delay_days smallint DEFAULT 2 NOT NULL,
    unpaid_threshold_mode text DEFAULT 'none'::text NOT NULL,
    CONSTRAINT circulation_settings_id_check CHECK ((id = 1)),
    CONSTRAINT circulation_settings_max_active_holds_check CHECK ((max_active_holds >= 1)),
    CONSTRAINT circulation_settings_pick_delay_days_check CHECK (((pick_delay_days >= 0) AND (pick_delay_days <= 365))),
    CONSTRAINT circulation_settings_unpaid_fine_threshold_check CHECK (((unpaid_fine_threshold)::numeric >= (0)::numeric)),
    CONSTRAINT circulation_settings_unpaid_threshold_mode_check CHECK ((unpaid_threshold_mode = ANY (ARRAY['none'::text, 'amount'::text])))
);

--
-- Name: COLUMN circulation_settings.unpaid_fine_threshold; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN circulation_settings.unpaid_fine_threshold IS 'Global unpaid-fine block in integer euro cents. Used only when unpaid_threshold_mode is amount.';

--
-- Name: COLUMN circulation_settings.max_active_holds; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN circulation_settings.max_active_holds IS 'Default maximum pending+ready holds per patron. public_types.max_active_holds overrides when set.';

--
-- Name: COLUMN circulation_settings.pick_delay_days; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN circulation_settings.pick_delay_days IS 'Days after pick_started_at before a to_pick row is flagged overdue on the pick list. Default 2. Does not expire or cancel the hold.';

--
-- Name: COLUMN circulation_settings.unpaid_threshold_mode; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN circulation_settings.unpaid_threshold_mode IS 'none: no amount-based block. amount: block when the unpaid balance is strictly above unpaid_fine_threshold. 0 blocks any positive unpaid balance.';

--
-- Name: collections; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE collections (
    id bigint NOT NULL,
    key character varying(255),
    name character varying(255) NOT NULL,
    secondary_title character varying(255),
    tertiary_title character varying(255),
    issn character varying(30),
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);

--
-- Name: collections_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE collections_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

--
-- Name: collections_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE collections_id_seq OWNED BY collections.id;

--
-- Name: deposit_batches; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE deposit_batches (
    id bigint NOT NULL,
    lender_id bigint NOT NULL,
    reference text NOT NULL,
    site_id bigint NOT NULL,
    arrived_on date NOT NULL,
    return_due_on date,
    status character varying(16) DEFAULT 'active'::character varying NOT NULL,
    closed_at timestamp with time zone,
    notes text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT deposit_batches_closed_at_check CHECK ((((status)::text = 'closed'::text) = (closed_at IS NOT NULL))),
    CONSTRAINT deposit_batches_return_due_on_check CHECK (((return_due_on IS NULL) OR (return_due_on >= arrived_on))),
    CONSTRAINT deposit_batches_status_check CHECK (((status)::text = ANY ((ARRAY['active'::character varying, 'returning'::character varying, 'closed'::character varying])::text[])))
);

--
-- Name: deposit_batches_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE deposit_batches_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

--
-- Name: deposit_batches_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE deposit_batches_id_seq OWNED BY deposit_batches.id;

--
-- Name: editions; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE editions (
    id bigint NOT NULL,
    key character varying(255),
    publisher_name character varying(255),
    place_of_publication character varying(255),
    notes text,
    date character varying(20),
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);

--
-- Name: editions_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE editions_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

--
-- Name: editions_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE editions_id_seq OWNED BY editions.id;

--
-- Name: email_outbox; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE email_outbox (
    id bigint NOT NULL,
    to_addr text NOT NULL,
    subject text NOT NULL,
    body text NOT NULL,
    status character varying(16) DEFAULT 'pending'::character varying NOT NULL,
    attempts integer DEFAULT 0 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    sent_at timestamp with time zone,
    CONSTRAINT email_outbox_status_chk CHECK (((status)::text = ANY ((ARRAY['pending'::character varying, 'sent'::character varying, 'failed'::character varying])::text[])))
);

--
-- Name: TABLE email_outbox; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON TABLE email_outbox IS 'Outbound email queue processed by the background scheduler.';

--
-- Name: COLUMN email_outbox.body; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN email_outbox.body IS 'JSON payload: {"plain":"...","html":"..."} for multipart send.';

--
-- Name: COLUMN email_outbox.status; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN email_outbox.status IS 'pending → sent, or failed after max delivery attempts.';

--
-- Name: COLUMN email_outbox.attempts; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN email_outbox.attempts IS 'Number of send attempts (incremented on transient failure).';

--
-- Name: email_outbox_event_announcements; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE email_outbox_event_announcements (
    outbox_id bigint NOT NULL,
    event_id bigint NOT NULL
);

--
-- Name: email_outbox_reminder_loans; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE email_outbox_reminder_loans (
    outbox_id bigint NOT NULL,
    loan_id bigint NOT NULL
);

--
-- Name: TABLE email_outbox_reminder_loans; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON TABLE email_outbox_reminder_loans IS 'Loans reserved by a pending overdue-reminder outbox row; released on sent or permanent failure.';

--
-- Name: email_templates; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE email_templates (
    template_id character varying(64) NOT NULL,
    language character varying(16) NOT NULL,
    subject text NOT NULL,
    body_plain text NOT NULL,
    body_html text,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    name character varying(128) NOT NULL,
    CONSTRAINT email_templates_language_chk CHECK (((language)::text = ANY ((ARRAY['english'::character varying, 'french'::character varying, 'german'::character varying, 'spanish'::character varying])::text[])))
);

--
-- Name: TABLE email_templates; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON TABLE email_templates IS 'Editable email templates. Populated from data/email_templates/*.json on first startup.';

--
-- Name: COLUMN email_templates.template_id; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN email_templates.template_id IS 'Logical id: 2fa_code, recovery_code, password_reset, hold_ready, overdue_reminder, event_announcement.';

--
-- Name: COLUMN email_templates.language; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN email_templates.language IS 'Language key matching models::Language (english, french, german, spanish).';

--
-- Name: COLUMN email_templates.name; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN email_templates.name IS 'Human-readable label shown in the Settings UI (localized per language row).';

--
-- Name: equipment; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE equipment (
    id bigint NOT NULL,
    name character varying(255) NOT NULL,
    equipment_type smallint DEFAULT 0 NOT NULL,
    has_internet boolean DEFAULT false,
    is_public boolean DEFAULT true,
    quantity integer DEFAULT 1,
    status smallint DEFAULT 0,
    notes character varying,
    created_at timestamp with time zone DEFAULT now(),
    update_at timestamp with time zone
);

--
-- Name: equipment_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE equipment_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

--
-- Name: equipment_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE equipment_id_seq OWNED BY equipment.id;

--
-- Name: event_audiences; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE event_audiences (
    event_id bigint NOT NULL,
    audience character varying(50) NOT NULL
);

--
-- Name: TABLE event_audiences; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON TABLE event_audiences IS 'Target audiences for an event (public_types.name). Empty only when events.all_audiences is true.';

--
-- Name: events; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE events (
    id bigint NOT NULL,
    name character varying(255) NOT NULL,
    event_type smallint DEFAULT 0 NOT NULL,
    event_date date NOT NULL,
    start_time time without time zone,
    end_time time without time zone,
    attendees_count integer DEFAULT 0,
    public_type character varying(50),
    school_name character varying(255),
    class_name character varying(255),
    students_count integer,
    partner_name character varying(255),
    description character varying,
    notes character varying,
    created_at timestamp with time zone DEFAULT now(),
    update_at timestamp with time zone,
    announcement_sent_at timestamp with time zone,
    attachment_data bytea,
    attachment_filename character varying(512),
    attachment_mime_type character varying(255),
    all_audiences boolean DEFAULT false NOT NULL
);

--
-- Name: COLUMN events.public_type; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN events.public_type IS 'Legacy single audience (public_types.name), nullable. Retained unused so this migration stays reversible. The application reads and writes all_audiences and event_audiences instead.';

--
-- Name: COLUMN events.attachment_data; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN events.attachment_data IS 'Optional attachment payload stored in-database';

--
-- Name: COLUMN events.attachment_filename; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN events.attachment_filename IS 'Original file name for Content-Disposition';

--
-- Name: COLUMN events.attachment_mime_type; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN events.attachment_mime_type IS 'MIME type for download Content-Type';

--
-- Name: COLUMN events.all_audiences; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN events.all_audiences IS 'True when the event targets every audience. event_audiences is empty in that case. False requires at least one event_audiences row.';

--
-- Name: events_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE events_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

--
-- Name: events_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE events_id_seq OWNED BY events.id;

--
-- Name: fees; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE fees (
    code character varying(50) NOT NULL,
    name character varying(100),
    amount integer DEFAULT 0
);

--
-- Name: fine_rules; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE fine_rules (
    id integer NOT NULL,
    media_type character varying(50),
    daily_rate bigint NOT NULL,
    max_amount bigint,
    grace_days integer DEFAULT 0 NOT NULL,
    notes text,
    CONSTRAINT fine_rules_daily_rate_check CHECK (((daily_rate)::numeric >= (0)::numeric)),
    CONSTRAINT fine_rules_grace_days_check CHECK ((grace_days >= 0)),
    CONSTRAINT fine_rules_max_amount_check CHECK (((max_amount IS NULL) OR ((max_amount)::numeric >= (0)::numeric)))
);

--
-- Name: COLUMN fine_rules.daily_rate; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN fine_rules.daily_rate IS 'Overdue daily rate in integer euro cents.';

--
-- Name: COLUMN fine_rules.max_amount; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN fine_rules.max_amount IS 'Overdue cap in integer euro cents. NULL means no cap.';

--
-- Name: fine_rules_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE fine_rules_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

--
-- Name: fine_rules_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE fine_rules_id_seq OWNED BY fine_rules.id;

--
-- Name: fines; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE fines (
    id bigint NOT NULL,
    loan_id bigint NOT NULL,
    user_id bigint,
    amount bigint NOT NULL,
    paid_amount bigint DEFAULT 0 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    paid_at timestamp with time zone,
    status character varying(20) DEFAULT 'pending'::character varying NOT NULL,
    notes text,
    charge_type character varying(20) DEFAULT 'overdue'::character varying NOT NULL,
    item_id bigint,
    refunded_amount bigint DEFAULT 0 NOT NULL,
    CONSTRAINT fines_amount_check CHECK (((amount)::numeric >= (0)::numeric)),
    CONSTRAINT fines_charge_type_check CHECK (((charge_type)::text = ANY ((ARRAY['overdue'::character varying, 'replacement'::character varying, 'damage'::character varying])::text[]))),
    CONSTRAINT fines_paid_amount_check CHECK (((paid_amount)::numeric >= (0)::numeric)),
    CONSTRAINT fines_paid_amount_lte_amount CHECK ((paid_amount <= amount)),
    CONSTRAINT fines_refunded_amount_check CHECK (((refunded_amount)::numeric >= (0)::numeric)),
    CONSTRAINT fines_refunded_amount_lte_paid CHECK ((refunded_amount <= paid_amount)),
    CONSTRAINT fines_status_check CHECK (((status)::text = ANY ((ARRAY['pending'::character varying, 'partial'::character varying, 'paid'::character varying, 'waived'::character varying, 'cancelled'::character varying, 'refunded'::character varying])::text[])))
);

--
-- Name: COLUMN fines.amount; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN fines.amount IS 'Charge total in integer euro cents.';

--
-- Name: COLUMN fines.paid_amount; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN fines.paid_amount IS 'Amount collected in integer euro cents.';

--
-- Name: COLUMN fines.status; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN fines.status IS 'pending, partial, paid, waived, cancelled (uncollected charge voided), or refunded (collected amount returned).';

--
-- Name: COLUMN fines.charge_type; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN fines.charge_type IS 'overdue (accrual), replacement (lost), or damage (repair/fee). One open fine per (loan, type).';

--
-- Name: COLUMN fines.item_id; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN fines.item_id IS 'Copy this charge belongs to. Set on replacement and damage bills so a lost copy can be resolved after the loan row is archived.';

--
-- Name: COLUMN fines.refunded_amount; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN fines.refunded_amount IS 'Amount given back in integer euro cents. Never exceeds paid_amount. paid_amount is kept, so paid_amount + refunded_amount can exceed amount after a refund.';

--
-- Name: genre_labels; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE genre_labels (
    genre_code smallint NOT NULL,
    language character varying(8) NOT NULL,
    label text NOT NULL,
    CONSTRAINT genre_labels_language_chk CHECK (((language)::text = ANY ((ARRAY['fr'::character varying, 'en'::character varying, 'de'::character varying, 'es'::character varying])::text[])))
);

--
-- Name: genre_synonyms; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE genre_synonyms (
    id bigint NOT NULL,
    genre_code smallint NOT NULL,
    term text NOT NULL,
    term_normalized text NOT NULL,
    match_kind text DEFAULT 'exact'::text NOT NULL,
    CONSTRAINT genre_synonyms_match_kind_chk CHECK ((match_kind = ANY (ARRAY['exact'::text, 'contains'::text])))
);

--
-- Name: genre_synonyms_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE genre_synonyms_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

--
-- Name: genre_synonyms_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE genre_synonyms_id_seq OWNED BY genre_synonyms.id;

--
-- Name: genres; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE genres (
    code smallint NOT NULL,
    shelf_code character varying(16),
    active boolean DEFAULT true NOT NULL,
    sort_order integer DEFAULT 0 NOT NULL
);

--
-- Name: TABLE genres; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON TABLE genres IS 'Library genre codes aligned with Genre enum SMALLINT values.';

--
-- Name: COLUMN genres.shelf_code; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN genres.shelf_code IS 'Call-number prefix when Dewey is absent; NULL = no genre shelf mark.';

--
-- Name: holds; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE holds (
    id bigint NOT NULL,
    user_id bigint NOT NULL,
    item_id bigint,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    notified_at timestamp with time zone,
    expires_at timestamp with time zone,
    status character varying(32) DEFAULT 'pending'::character varying NOT NULL,
    "position" integer DEFAULT 1 NOT NULL,
    biblio_id bigint NOT NULL,
    pickup_site_id bigint,
    pick_started_at timestamp with time zone,
    patron_note text,
    internal_note text,
    CONSTRAINT holds_status_chk CHECK (((status)::text = ANY ((ARRAY['pending'::character varying, 'to_pick'::character varying, 'ready'::character varying, 'fulfilled'::character varying, 'cancelled'::character varying, 'expired'::character varying])::text[])))
);

--
-- Name: COLUMN holds.item_id; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN holds.item_id IS 'Allocated copy. NULL while waiting in the title queue; set when staff pin a specimen or when fulfillment assigns one.';

--
-- Name: COLUMN holds.status; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN holds.status IS 'pending: waiting for a copy. to_pick: copy assigned, still on the shelf (no patron notice). ready: patron notified. fulfilled / cancelled / expired: closed.';

--
-- Name: COLUMN holds.biblio_id; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN holds.biblio_id IS 'Bibliographic record (notice) this hold is queued on. Always set.';

--
-- Name: COLUMN holds.pickup_site_id; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN holds.pickup_site_id IS 'Pickup site (sources.id). When set and different from the allocated copy''s current source, fulfillment goes through item transit.';

--
-- Name: COLUMN holds.pick_started_at; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN holds.pick_started_at IS 'When the current to_pick assignment started. Cleared when the hold leaves to_pick. Due date is this plus circulation_settings.pick_delay_days. No automatic expiry.';

--
-- Name: COLUMN holds.patron_note; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN holds.patron_note IS 'Note visible to the patron and to staff. Former holds.notes values are not copied here.';

--
-- Name: COLUMN holds.internal_note; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN holds.internal_note IS 'Staff-only hold note. Former holds.notes values live here.';

--
-- Name: idempotency_keys; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE idempotency_keys (
    actor_id bigint NOT NULL,
    idempotency_key character varying(255) NOT NULL,
    operation character varying(64) NOT NULL,
    request_hash character(64) NOT NULL,
    status_code smallint,
    response_body jsonb,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    expires_at timestamp with time zone NOT NULL
);

--
-- Name: TABLE idempotency_keys; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON TABLE idempotency_keys IS 'Replay store for Idempotency-Key: same actor+key+payload returns the stored response within TTL.';

--
-- Name: COLUMN idempotency_keys.request_hash; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN idempotency_keys.request_hash IS 'SHA-256 hex of the canonical request fingerprint (operation + payload).';

--
-- Name: COLUMN idempotency_keys.status_code; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN idempotency_keys.status_code IS 'NULL while the first request is in flight; set when a 2xx body is stored.';

--
-- Name: inventory_scans; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE inventory_scans (
    id bigint NOT NULL,
    session_id bigint NOT NULL,
    item_id bigint,
    barcode character varying(100) NOT NULL,
    scanned_at timestamp with time zone DEFAULT now() NOT NULL,
    result character varying(32) NOT NULL,
    scanned_by bigint,
    CONSTRAINT inventory_scans_result_chk CHECK (((result)::text = ANY ((ARRAY['found'::character varying, 'unknown_barcode'::character varying, 'found_archived'::character varying, 'found_out_of_scope'::character varying])::text[])))
);

--
-- Name: inventory_scans_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE inventory_scans_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

--
-- Name: inventory_scans_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE inventory_scans_id_seq OWNED BY inventory_scans.id;

--
-- Name: inventory_sessions; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE inventory_sessions (
    id bigint NOT NULL,
    name text NOT NULL,
    started_at timestamp with time zone DEFAULT now() NOT NULL,
    closed_at timestamp with time zone,
    status character varying(16) DEFAULT 'open'::character varying NOT NULL,
    location_filter text,
    notes text,
    scope_place smallint,
    created_by bigint,
    consolidated_at timestamp with time zone,
    consolidated_by bigint,
    scope_site_id bigint,
    CONSTRAINT inventory_sessions_status_chk CHECK (((status)::text = ANY ((ARRAY['open'::character varying, 'closed'::character varying])::text[])))
);

--
-- Name: COLUMN inventory_sessions.scope_site_id; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN inventory_sessions.scope_site_id IS 'When set, expected counts, missing list, scans, and consolidation only include active items with this site_id.';

--
-- Name: item_transits; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE item_transits (
    id bigint NOT NULL,
    item_id bigint NOT NULL,
    hold_id bigint,
    from_site_id bigint NOT NULL,
    to_site_id bigint NOT NULL,
    status character varying(20) DEFAULT 'requested'::character varying NOT NULL,
    notes text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    shipped_at timestamp with time zone,
    received_at timestamp with time zone,
    cancelled_at timestamp with time zone,
    shipped_by bigint,
    received_by bigint,
    cancelled_by bigint,
    reversed_from_id bigint,
    CONSTRAINT item_transits_sites_differ CHECK ((from_site_id <> to_site_id)),
    CONSTRAINT item_transits_status_check CHECK (((status)::text = ANY ((ARRAY['requested'::character varying, 'in_transit'::character varying, 'received'::character varying, 'cancelled'::character varying])::text[])))
);

--
-- Name: TABLE item_transits; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON TABLE item_transits IS 'Inter-site item movement for hold fulfillment. Not a reservation type; the hold queue remains the reservation.';

--
-- Name: COLUMN item_transits.status; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN item_transits.status IS 'requested → in_transit → received. cancelled frees the copy; a reverse row may be created.';

--
-- Name: item_withdrawals; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE item_withdrawals (
    id bigint NOT NULL,
    item_id bigint NOT NULL,
    withdrawn_at timestamp with time zone DEFAULT now() NOT NULL,
    reason character varying(32) NOT NULL,
    reason_detail text,
    destination character varying(32),
    agent_id bigint,
    cancelled_at timestamp with time zone,
    cancelled_by bigint,
    disposed_at timestamp with time zone,
    disposed_by bigint,
    legacy boolean DEFAULT false NOT NULL,
    CONSTRAINT item_withdrawals_destination_chk CHECK (((destination IS NULL) OR ((destination)::text = ANY ((ARRAY['pulp'::character varying, 'donation'::character varying, 'sale'::character varying, 'deposit'::character varying])::text[])))),
    CONSTRAINT item_withdrawals_destination_required_chk CHECK ((legacy OR (destination IS NOT NULL))),
    CONSTRAINT item_withdrawals_other_detail_chk CHECK ((((reason)::text <> 'other'::text) OR ((reason_detail IS NOT NULL) AND (char_length(btrim(reason_detail)) >= 3)))),
    CONSTRAINT item_withdrawals_reason_chk CHECK (((reason)::text = ANY ((ARRAY['worn'::character varying, 'obsolete'::character varying, 'new_edition'::character varying, 'surplus'::character varying, 'not_borrowed'::character varying, 'out_of_policy'::character varying, 'lost_not_replaced'::character varying, 'other'::character varying])::text[])))
);

--
-- Name: TABLE item_withdrawals; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON TABLE item_withdrawals IS 'Weeding event for one physical copy. Cancel keeps the row. Legacy rows are the #56/#68 backfill (destination may be null).';

--
-- Name: COLUMN item_withdrawals.destination; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN item_withdrawals.destination IS 'pulp, donation, sale, or deposit. NULL only on legacy rows that were not yet completed.';

--
-- Name: COLUMN item_withdrawals.disposed_at; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN item_withdrawals.disposed_at IS 'When the copy physically left the library. NULL until staff record disposal. Legacy #56/#68 rows use withdrawn_at.';

--
-- Name: COLUMN item_withdrawals.disposed_by; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN item_withdrawals.disposed_by IS 'Staff member who recorded disposal. NULL on the #56/#68 backfill and until disposal is recorded.';

--
-- Name: COLUMN item_withdrawals.legacy; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN item_withdrawals.legacy IS 'True when the row was backfilled from items.weeding_status/weeding_reason. Destination may be completed later.';

--
-- Name: items; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE items (
    id bigint NOT NULL,
    biblio_id bigint,
    site_id bigint,
    barcode character varying(100),
    call_number character varying(100),
    volume_designation text,
    place smallint,
    borrowable boolean DEFAULT true NOT NULL,
    circulation_status smallint,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone,
    archived_at timestamp with time zone,
    weeding_status character varying(20) DEFAULT 'on_shelf'::character varying NOT NULL,
    weeding_reason character varying(200),
    public_note text,
    internal_note text,
    price_deferred boolean DEFAULT false NOT NULL,
    price bigint,
    deposit_batch_id bigint,
    deposit_returned_at timestamp with time zone,
    deposit_loss_notified_at timestamp with time zone,
    lender_barcode character varying(100),
    CONSTRAINT items_deposit_loss_notified_needs_batch CHECK (((deposit_loss_notified_at IS NULL) OR (deposit_batch_id IS NOT NULL))),
    CONSTRAINT items_deposit_returned_needs_batch CHECK (((deposit_returned_at IS NULL) OR (deposit_batch_id IS NOT NULL))),
    CONSTRAINT items_price_nonnegative CHECK (((price IS NULL) OR (price >= 0))),
    CONSTRAINT items_weeding_status_chk CHECK (((weeding_status)::text = ANY ((ARRAY['on_shelf'::character varying, 'candidate'::character varying, 'withdrawn'::character varying])::text[])))
);

--
-- Name: COLUMN items.site_id; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN items.site_id IS 'Physical site where the copy is held (sites.id).';

--
-- Name: COLUMN items.circulation_status; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN items.circulation_status IS 'Item-level state machine: NULL/0 available, 1 lost, 2 damaged, 3 claimed-returned. Independent of the loan row.';

--
-- Name: COLUMN items.weeding_status; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN items.weeding_status IS 'Weeding lifecycle, orthogonal to circulation_status and archived_at: on_shelf (default), candidate (still circulable), withdrawn (not circulable).';

--
-- Name: COLUMN items.weeding_reason; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN items.weeding_reason IS 'Optional short staff reason for the current weeding status.';

--
-- Name: COLUMN items.public_note; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN items.public_note IS 'Copy note shown to patrons and the OPAC (for example "CD missing").';

--
-- Name: COLUMN items.internal_note; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN items.internal_note IS 'Staff-only copy note. Former items.notes values live here.';

--
-- Name: COLUMN items.price_deferred; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN items.price_deferred IS 'True when a missing price is an explicit deferral, including a legacy price rejected by the cents migration, so the copy can stay in circulation.';

--
-- Name: COLUMN items.price; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN items.price IS 'Purchase price in integer euro cents. NULL means unknown. 0 is a free copy, not unknown.';

--
-- Name: items_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE items_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

--
-- Name: items_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE items_id_seq OWNED BY items.id;

--
-- Name: lenders; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE lenders (
    id bigint NOT NULL,
    name text NOT NULL,
    contact text,
    notes text,
    archived_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    replacement_rate_cents bigint,
    CONSTRAINT lenders_replacement_rate_positive CHECK (((replacement_rate_cents IS NULL) OR (replacement_rate_cents > 0)))
);

--
-- Name: lenders_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE lenders_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

--
-- Name: lenders_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE lenders_id_seq OWNED BY lenders.id;

--
-- Name: library_info; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE library_info (
    id smallint DEFAULT 1 NOT NULL,
    name text,
    addr_line1 character varying(100),
    addr_line2 character varying(100),
    addr_postcode character varying(10),
    addr_city character varying(100),
    addr_country character varying(50),
    phones jsonb DEFAULT '[]'::jsonb NOT NULL,
    email text,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    timezone text DEFAULT 'Europe/Paris'::text NOT NULL,
    language text,
    CONSTRAINT library_info_language_check CHECK (((language IS NULL) OR (language = ANY (ARRAY['fr'::text, 'en'::text, 'de'::text, 'es'::text])))),
    CONSTRAINT library_info_single_row CHECK ((id = 1))
);

--
-- Name: COLUMN library_info.addr_country; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN library_info.addr_country IS 'ISO 3166-1 alpha-2 country code (for example FR). Unrecognized legacy free text is cleared so the next settings save must choose a country.';

--
-- Name: COLUMN library_info.timezone; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN library_info.timezone IS 'IANA timezone name (for example Europe/Paris) used for patron-facing dates and open-day calculations.';

--
-- Name: COLUMN library_info.language; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN library_info.language IS 'Default public UI language: fr, en, de, or es. NULL means unset (clients fall back to fr).';

--
-- Name: loans; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE loans (
    id bigint NOT NULL,
    user_id bigint NOT NULL,
    item_id bigint,
    date timestamp with time zone DEFAULT now() NOT NULL,
    renew_at timestamp with time zone,
    nb_renews smallint DEFAULT 0,
    expiry_at timestamp with time zone,
    notes text,
    returned_at timestamp with time zone,
    last_reminder_sent_at timestamp with time zone,
    reminder_count integer DEFAULT 0 NOT NULL,
    site_id bigint
);

--
-- Name: COLUMN loans.site_id; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN loans.site_id IS 'Site where the loan was made. NULL when the loan predates this column or the request omitted the desk; calendar and statistics then use the library default site (sources."default"), not items.source_id.';

--
-- Name: loans_archives; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE loans_archives (
    id bigint NOT NULL,
    user_id bigint,
    item_id bigint,
    date timestamp with time zone,
    nb_renews smallint DEFAULT 0,
    expiry_at timestamp with time zone,
    returned_at timestamp with time zone,
    notes text,
    borrower_public_type bigint,
    addr_city character varying(255),
    account_type character varying(50),
    age_band character varying(16),
    loan_year smallint,
    closure_reason text DEFAULT 'returned'::text NOT NULL,
    found_at timestamp with time zone,
    site_id bigint,
    CONSTRAINT loans_archives_closure_reason_check CHECK ((closure_reason = ANY (ARRAY['returned'::text, 'lost'::text])))
);

--
-- Name: COLUMN loans_archives.age_band; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN loans_archives.age_band IS 'Non-identifying age band at loan date (0-17, 18-29, 30-49, 50-64, 65+). Snapshotted for stats after patron erasure.';

--
-- Name: COLUMN loans_archives.loan_year; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN loans_archives.loan_year IS 'Calendar year of the loan date; stats dimension retained after user_id is nulled.';

--
-- Name: COLUMN loans_archives.closure_reason; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN loans_archives.closure_reason IS 'Why the loan was closed: returned (physical return, damaged return, or claims-returned found) or lost (declared lost, or claims-returned not found). Finding the copy later does not change this.';

--
-- Name: COLUMN loans_archives.found_at; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN loans_archives.found_at IS 'When a copy closed as lost was later accepted back (circulate or damaged). Null while it stays lost, and null for returned loans.';

--
-- Name: COLUMN loans_archives.site_id; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN loans_archives.site_id IS 'Copied from loans.site_id when the loan is archived. NULL when that loan had no recorded site; readers then use the library default site. Not backfilled. ON DELETE RESTRICT.';

--
-- Name: loans_archives_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE loans_archives_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

--
-- Name: loans_archives_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE loans_archives_id_seq OWNED BY loans_archives.id;

--
-- Name: loans_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE loans_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

--
-- Name: loans_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE loans_id_seq OWNED BY loans.id;

--
-- Name: loans_settings; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE loans_settings (
    id bigint NOT NULL,
    media_type character varying(30),
    nb_max smallint,
    nb_renews smallint,
    duration smallint,
    notes text,
    renew_at character varying(32) DEFAULT 'at_due_date'::character varying NOT NULL,
    renew_window_days smallint DEFAULT 7 NOT NULL,
    CONSTRAINT loans_settings_renew_at_chk CHECK (((renew_at)::text = ANY ((ARRAY['now'::character varying, 'at_due_date'::character varying])::text[]))),
    CONSTRAINT loans_settings_renew_window_days_chk CHECK (((renew_window_days >= 0) AND (renew_window_days <= 366)))
);

--
-- Name: COLUMN loans_settings.renew_at; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN loans_settings.renew_at IS 'Renewal due-date anchor: at_due_date (default) = current expiry + duration; now = renewal time + duration.';

--
-- Name: COLUMN loans_settings.renew_window_days; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN loans_settings.renew_window_days IS 'Days before the due date when renewal becomes allowed. 0 = on the due date or later. Overdue loans stay renewable.';

--
-- Name: loans_settings_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE loans_settings_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

--
-- Name: loans_settings_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE loans_settings_id_seq OWNED BY loans_settings.id;

--
-- Name: public_type_loan_settings; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public_type_loan_settings (
    id bigint NOT NULL,
    public_type_id bigint NOT NULL,
    media_type character varying(50),
    duration smallint,
    nb_max smallint,
    nb_renews smallint,
    renew_at character varying(32),
    CONSTRAINT public_type_loan_settings_renew_at_chk CHECK (((renew_at IS NULL) OR ((renew_at)::text = ANY ((ARRAY['now'::character varying, 'at_due_date'::character varying])::text[]))))
);

--
-- Name: COLUMN public_type_loan_settings.renew_at; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public_type_loan_settings.renew_at IS 'Renewal anchor override: NULL = use loans_settings.renew_at for this media type; otherwise now | at_due_date.';

--
-- Name: public_type_loan_settings_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public_type_loan_settings_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

--
-- Name: public_type_loan_settings_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public_type_loan_settings_id_seq OWNED BY public_type_loan_settings.id;

--
-- Name: public_types; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public_types (
    id bigint NOT NULL,
    name character varying(50) NOT NULL,
    label character varying(100) NOT NULL,
    subscription_duration_days integer DEFAULT 365,
    age_min smallint,
    age_max smallint,
    subscription_price bigint DEFAULT 0,
    unpaid_fine_threshold bigint,
    max_active_holds smallint,
    unpaid_threshold_mode text DEFAULT 'inherit'::text NOT NULL,
    CONSTRAINT public_types_max_active_holds_check CHECK (((max_active_holds IS NULL) OR (max_active_holds >= 1))),
    CONSTRAINT public_types_subscription_price_nonnegative CHECK (((subscription_price IS NULL) OR (subscription_price >= 0))),
    CONSTRAINT public_types_unpaid_fine_threshold_check CHECK (((unpaid_fine_threshold IS NULL) OR ((unpaid_fine_threshold)::numeric >= (0)::numeric))),
    CONSTRAINT public_types_unpaid_threshold_mode_check CHECK ((unpaid_threshold_mode = ANY (ARRAY['inherit'::text, 'none'::text, 'amount'::text])))
);

--
-- Name: COLUMN public_types.subscription_price; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public_types.subscription_price IS 'Subscription price in integer euro cents (1500 = 15.00 EUR). Not a major-unit amount.';

--
-- Name: COLUMN public_types.unpaid_fine_threshold; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public_types.unpaid_fine_threshold IS 'Unpaid-fine block in integer euro cents. Used only when unpaid_threshold_mode is amount. 0 blocks any positive unpaid balance.';

--
-- Name: COLUMN public_types.max_active_holds; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public_types.max_active_holds IS 'Optional max pending+ready holds for this audience. NULL inherits circulation_settings.max_active_holds.';

--
-- Name: COLUMN public_types.unpaid_threshold_mode; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public_types.unpaid_threshold_mode IS 'inherit: use the global circulation mode. none: never block on amount, even when the global mode is amount. amount: block when the unpaid balance is strictly above unpaid_fine_threshold.';

--
-- Name: public_types_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public_types_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

--
-- Name: public_types_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public_types_id_seq OWNED BY public_types.id;

--
-- Name: purchase_order_lines; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE purchase_order_lines (
    id bigint NOT NULL,
    purchase_order_id bigint NOT NULL,
    fund_id bigint,
    biblio_id bigint,
    isbn character varying(30),
    title character varying(500),
    quantity_ordered integer NOT NULL,
    quantity_received integer DEFAULT 0 NOT NULL,
    unit_price numeric(14,2),
    currency character varying(3) DEFAULT 'EUR'::character varying NOT NULL,
    notes text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT purchase_order_lines_qty_le_ordered_chk CHECK ((quantity_received <= quantity_ordered)),
    CONSTRAINT purchase_order_lines_qty_ordered_chk CHECK ((quantity_ordered > 0)),
    CONSTRAINT purchase_order_lines_qty_received_chk CHECK ((quantity_received >= 0))
);

--
-- Name: purchase_orders; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE purchase_orders (
    id bigint NOT NULL,
    vendor_id bigint,
    fund_id bigint,
    order_number character varying(50) NOT NULL,
    status character varying(20) DEFAULT 'draft'::character varying NOT NULL,
    notes text,
    ordered_at timestamp with time zone,
    created_by bigint,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);

--
-- Name: purchase_suggestions; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE purchase_suggestions (
    id bigint NOT NULL,
    proposed_by bigint NOT NULL,
    title character varying(500) NOT NULL,
    author character varying(300) NOT NULL,
    comment text,
    status character varying(20) DEFAULT 'proposed'::character varying NOT NULL,
    staff_note text,
    reviewed_by bigint,
    reviewed_at timestamp with time zone,
    purchase_order_id bigint,
    purchase_order_line_id bigint,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT purchase_suggestions_author_chk CHECK ((length(btrim((author)::text)) > 0)),
    CONSTRAINT purchase_suggestions_status_chk CHECK (((status)::text = ANY ((ARRAY['proposed'::character varying, 'accepted'::character varying, 'refused'::character varying])::text[]))),
    CONSTRAINT purchase_suggestions_title_chk CHECK ((length(btrim((title)::text)) > 0))
);

--
-- Name: TABLE purchase_suggestions; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON TABLE purchase_suggestions IS 'Patron title proposals. Accept opens a draft purchase-order line (intent); refuse stays refused. No biblio or item is created here.';

--
-- Name: receipt_items; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE receipt_items (
    receipt_line_id bigint NOT NULL,
    item_id bigint NOT NULL
);

--
-- Name: receipt_lines; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE receipt_lines (
    id bigint NOT NULL,
    receipt_id bigint NOT NULL,
    purchase_order_line_id bigint NOT NULL,
    quantity integer NOT NULL,
    unit_price numeric(14,2),
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT receipt_lines_qty_chk CHECK ((quantity > 0))
);

--
-- Name: receipts; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE receipts (
    id bigint NOT NULL,
    purchase_order_id bigint NOT NULL,
    received_at timestamp with time zone DEFAULT now() NOT NULL,
    received_by bigint,
    notes text,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);

--
-- Name: retired_card_numbers; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE retired_card_numbers (
    id bigint NOT NULL,
    card_number character varying(100) NOT NULL,
    user_id bigint NOT NULL,
    retired_at timestamp with time zone DEFAULT now() NOT NULL,
    retired_by bigint
);

--
-- Name: TABLE retired_card_numbers; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON TABLE retired_card_numbers IS 'Library card numbers taken off a patron by card replacement. Never assign these values again.';

--
-- Name: COLUMN retired_card_numbers.card_number; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN retired_card_numbers.card_number IS 'Exact previous users.barcode. Comparison is case-sensitive, matching users.barcode.';

--
-- Name: COLUMN retired_card_numbers.retired_by; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN retired_card_numbers.retired_by IS 'Staff user who replaced the card. Null only if that account is removed later.';

--
-- Name: saved_queries; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE saved_queries (
    id bigint NOT NULL,
    name character varying(200) NOT NULL,
    description text,
    query_json jsonb NOT NULL,
    user_id bigint NOT NULL,
    is_shared boolean DEFAULT false NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);

--
-- Name: saved_queries_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE saved_queries_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

--
-- Name: saved_queries_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE saved_queries_id_seq OWNED BY saved_queries.id;

--
-- Name: schedule_closures; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE schedule_closures (
    id bigint NOT NULL,
    closure_date date NOT NULL,
    reason character varying,
    created_at timestamp with time zone DEFAULT now(),
    site_id bigint
);

--
-- Name: COLUMN schedule_closures.site_id; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN schedule_closures.site_id IS 'NULL = network-wide closure (every site). Set = this site only (sites.id).';

--
-- Name: schedule_closures_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE schedule_closures_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

--
-- Name: schedule_closures_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE schedule_closures_id_seq OWNED BY schedule_closures.id;

--
-- Name: schedule_periods; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE schedule_periods (
    id bigint NOT NULL,
    name character varying(100) NOT NULL,
    start_date date NOT NULL,
    end_date date NOT NULL,
    notes character varying,
    created_at timestamp with time zone DEFAULT now(),
    update_at timestamp with time zone
);

--
-- Name: schedule_periods_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE schedule_periods_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

--
-- Name: schedule_periods_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE schedule_periods_id_seq OWNED BY schedule_periods.id;

--
-- Name: schedule_slots; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE schedule_slots (
    id bigint NOT NULL,
    period_id bigint NOT NULL,
    day_of_week smallint NOT NULL,
    open_time time without time zone NOT NULL,
    close_time time without time zone NOT NULL,
    created_at timestamp with time zone DEFAULT now(),
    CONSTRAINT schedule_slots_day_of_week_check CHECK (((day_of_week >= 0) AND (day_of_week <= 6)))
);

--
-- Name: schedule_slots_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE schedule_slots_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

--
-- Name: schedule_slots_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE schedule_slots_id_seq OWNED BY schedule_slots.id;

--
-- Name: series; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE series (
    id bigint NOT NULL,
    key character varying(255),
    name character varying(255) NOT NULL,
    issn character varying(30),
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);

--
-- Name: series_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE series_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

--
-- Name: series_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE series_id_seq OWNED BY series.id;

--
-- Name: settings; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE settings (
    key character varying(100) NOT NULL,
    value jsonb NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);

--
-- Name: sites; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE sites (
    id bigint NOT NULL,
    key character varying(255),
    name character varying(255),
    is_archive smallint DEFAULT 0,
    archived_at timestamp with time zone,
    "default" boolean DEFAULT false
);

--
-- Name: sites_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE sites_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

--
-- Name: sites_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE sites_id_seq OWNED BY sites.id;

--
-- Name: user_guardians; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE user_guardians (
    child_id bigint NOT NULL,
    guardian_id bigint NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT user_guardians_not_self CHECK ((child_id <> guardian_id))
);

--
-- Name: TABLE user_guardians; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON TABLE user_guardians IS 'Legal guardian link: child_id is a minor patron (public type child or school); guardian_id is another patron.';

--
-- Name: users; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE users (
    id bigint NOT NULL,
    login character varying(255),
    password character varying(255),
    firstname character varying(255),
    lastname character varying(255),
    email character varying(255),
    addr_street character varying(255),
    addr_zip_code integer,
    addr_city character varying(255),
    phone character varying(50),
    account_type character varying(50) DEFAULT 'guest'::character varying NOT NULL,
    fee character varying(50),
    group_id bigint,
    barcode character varying(100),
    notes text,
    public_type bigint,
    status character varying(32) DEFAULT 'active'::character varying,
    birthdate date,
    created_at timestamp with time zone DEFAULT now(),
    update_at timestamp with time zone,
    expiry_at timestamp with time zone,
    archived_at timestamp with time zone,
    language character varying(32),
    sex character varying(1),
    staff_type smallint,
    hours_per_week real,
    staff_start_date date,
    staff_end_date date,
    receive_reminders boolean DEFAULT true NOT NULL,
    two_factor_enabled boolean DEFAULT false,
    two_factor_method character varying(20),
    totp_secret text,
    recovery_codes text,
    recovery_codes_used text,
    must_change_password boolean DEFAULT false NOT NULL,
    token_version bigint DEFAULT 0 NOT NULL,
    events_consent_at timestamp with time zone,
    events_consent_source character varying(20),
    events_consent_changed_at timestamp with time zone,
    events_consent_notice_at timestamp with time zone,
    password_reset_nonce bigint DEFAULT 0 NOT NULL,
    card_lost boolean DEFAULT false NOT NULL,
    CONSTRAINT users_events_consent_source_check CHECK (((events_consent_source IS NULL) OR ((events_consent_source)::text = ANY ((ARRAY['registration'::character varying, 'desk'::character varying, 'profile'::character varying, 'migration'::character varying, 'unsubscribe'::character varying])::text[])))),
    CONSTRAINT users_sex_check CHECK (((sex IS NULL) OR ((sex)::text = ANY ((ARRAY['m'::character varying, 'f'::character varying])::text[]))))
);

--
-- Name: COLUMN users.password_reset_nonce; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN users.password_reset_nonce IS 'Invalidates outstanding password-reset links. Incremented when a link is issued, when a reset succeeds, and when the password changes.';

--
-- Name: COLUMN users.card_lost; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN users.card_lost IS 'True when this patron''s library card is declared lost or stolen. Checkout always refuses with 422 card_lost; force does not override.';

--
-- Name: users_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE users_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

--
-- Name: users_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE users_id_seq OWNED BY users.id;

--
-- Name: vendors; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE vendors (
    id bigint NOT NULL,
    name character varying(200) NOT NULL,
    code character varying(50),
    email character varying(200),
    phone character varying(50),
    address text,
    notes text,
    active boolean DEFAULT true NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    archived_at timestamp with time zone
);

--
-- Name: visitor_counts; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE visitor_counts (
    id bigint NOT NULL,
    count_date date NOT NULL,
    count integer DEFAULT 0 NOT NULL,
    source character varying(50) DEFAULT 'manual'::character varying,
    notes character varying,
    created_at timestamp with time zone DEFAULT now()
);

--
-- Name: visitor_counts_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE visitor_counts_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

--
-- Name: visitor_counts_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE visitor_counts_id_seq OWNED BY visitor_counts.id;

--
-- Name: z3950servers; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE z3950servers (
    id bigint NOT NULL,
    address character varying(255),
    port integer,
    name character varying(255),
    description text,
    activated boolean DEFAULT true,
    login character varying(255),
    password character varying(255),
    database character varying(255),
    format character varying(50),
    encoding character varying(20) DEFAULT 'utf-8'::character varying
);

--
-- Name: z3950servers_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE z3950servers_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

--
-- Name: z3950servers_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE z3950servers_id_seq OWNED BY z3950servers.id;

--
-- Name: audit_log id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY audit_log ALTER COLUMN id SET DEFAULT nextval('audit_log_id_seq'::regclass);

--
-- Name: authors id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY authors ALTER COLUMN id SET DEFAULT nextval('authors_id_seq'::regclass);

--
-- Name: biblio_authors id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY biblio_authors ALTER COLUMN id SET DEFAULT nextval('biblio_authors_id_seq'::regclass);

--
-- Name: biblio_collections id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY biblio_collections ALTER COLUMN id SET DEFAULT nextval('biblio_collections_id_seq'::regclass);

--
-- Name: biblio_series id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY biblio_series ALTER COLUMN id SET DEFAULT nextval('biblio_series_id_seq'::regclass);

--
-- Name: biblios id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY biblios ALTER COLUMN id SET DEFAULT nextval('biblios_id_seq'::regclass);

--
-- Name: collections id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY collections ALTER COLUMN id SET DEFAULT nextval('collections_id_seq'::regclass);

--
-- Name: deposit_batches id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY deposit_batches ALTER COLUMN id SET DEFAULT nextval('deposit_batches_id_seq'::regclass);

--
-- Name: editions id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY editions ALTER COLUMN id SET DEFAULT nextval('editions_id_seq'::regclass);

--
-- Name: equipment id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY equipment ALTER COLUMN id SET DEFAULT nextval('equipment_id_seq'::regclass);

--
-- Name: events id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY events ALTER COLUMN id SET DEFAULT nextval('events_id_seq'::regclass);

--
-- Name: fine_rules id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY fine_rules ALTER COLUMN id SET DEFAULT nextval('fine_rules_id_seq'::regclass);

--
-- Name: genre_synonyms id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY genre_synonyms ALTER COLUMN id SET DEFAULT nextval('genre_synonyms_id_seq'::regclass);

--
-- Name: inventory_scans id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY inventory_scans ALTER COLUMN id SET DEFAULT nextval('inventory_scans_id_seq'::regclass);

--
-- Name: items id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY items ALTER COLUMN id SET DEFAULT nextval('items_id_seq'::regclass);

--
-- Name: lenders id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY lenders ALTER COLUMN id SET DEFAULT nextval('lenders_id_seq'::regclass);

--
-- Name: loans id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY loans ALTER COLUMN id SET DEFAULT nextval('loans_id_seq'::regclass);

--
-- Name: loans_archives id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY loans_archives ALTER COLUMN id SET DEFAULT nextval('loans_archives_id_seq'::regclass);

--
-- Name: loans_settings id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY loans_settings ALTER COLUMN id SET DEFAULT nextval('loans_settings_id_seq'::regclass);

--
-- Name: public_type_loan_settings id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public_type_loan_settings ALTER COLUMN id SET DEFAULT nextval('public_type_loan_settings_id_seq'::regclass);

--
-- Name: public_types id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public_types ALTER COLUMN id SET DEFAULT nextval('public_types_id_seq'::regclass);

--
-- Name: saved_queries id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY saved_queries ALTER COLUMN id SET DEFAULT nextval('saved_queries_id_seq'::regclass);

--
-- Name: schedule_closures id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY schedule_closures ALTER COLUMN id SET DEFAULT nextval('schedule_closures_id_seq'::regclass);

--
-- Name: schedule_periods id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY schedule_periods ALTER COLUMN id SET DEFAULT nextval('schedule_periods_id_seq'::regclass);

--
-- Name: schedule_slots id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY schedule_slots ALTER COLUMN id SET DEFAULT nextval('schedule_slots_id_seq'::regclass);

--
-- Name: series id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY series ALTER COLUMN id SET DEFAULT nextval('series_id_seq'::regclass);

--
-- Name: sites id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY sites ALTER COLUMN id SET DEFAULT nextval('sites_id_seq'::regclass);

--
-- Name: users id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY users ALTER COLUMN id SET DEFAULT nextval('users_id_seq'::regclass);

--
-- Name: visitor_counts id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY visitor_counts ALTER COLUMN id SET DEFAULT nextval('visitor_counts_id_seq'::regclass);

--
-- Name: z3950servers id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY z3950servers ALTER COLUMN id SET DEFAULT nextval('z3950servers_id_seq'::regclass);

--
-- Name: account_types account_types_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY account_types
    ADD CONSTRAINT account_types_pkey PRIMARY KEY (code);

--
-- Name: acquisition_funds acquisition_funds_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY acquisition_funds
    ADD CONSTRAINT acquisition_funds_pkey PRIMARY KEY (id);

--
-- Name: audit_log audit_log_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY audit_log
    ADD CONSTRAINT audit_log_pkey PRIMARY KEY (id);

--
-- Name: authors authors_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY authors
    ADD CONSTRAINT authors_pkey PRIMARY KEY (id);

--
-- Name: biblio_authors biblio_authors_biblio_id_author_id_function_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY biblio_authors
    ADD CONSTRAINT biblio_authors_biblio_id_author_id_function_key UNIQUE (biblio_id, author_id, function);

--
-- Name: biblio_authors biblio_authors_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY biblio_authors
    ADD CONSTRAINT biblio_authors_pkey PRIMARY KEY (id);

--
-- Name: biblio_collections biblio_collections_biblio_id_collection_id_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY biblio_collections
    ADD CONSTRAINT biblio_collections_biblio_id_collection_id_key UNIQUE (biblio_id, collection_id);

--
-- Name: biblio_collections biblio_collections_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY biblio_collections
    ADD CONSTRAINT biblio_collections_pkey PRIMARY KEY (id);

--
-- Name: biblio_series biblio_series_biblio_id_series_id_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY biblio_series
    ADD CONSTRAINT biblio_series_biblio_id_series_id_key UNIQUE (biblio_id, series_id);

--
-- Name: biblio_series biblio_series_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY biblio_series
    ADD CONSTRAINT biblio_series_pkey PRIMARY KEY (id);

--
-- Name: biblios biblios_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY biblios
    ADD CONSTRAINT biblios_pkey PRIMARY KEY (id);

--
-- Name: circulation_settings circulation_settings_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY circulation_settings
    ADD CONSTRAINT circulation_settings_pkey PRIMARY KEY (id);

--
-- Name: collections collections_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY collections
    ADD CONSTRAINT collections_pkey PRIMARY KEY (id);

--
-- Name: deposit_batches deposit_batches_lender_id_reference_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY deposit_batches
    ADD CONSTRAINT deposit_batches_lender_id_reference_key UNIQUE (lender_id, reference);

--
-- Name: deposit_batches deposit_batches_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY deposit_batches
    ADD CONSTRAINT deposit_batches_pkey PRIMARY KEY (id);

--
-- Name: editions editions_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY editions
    ADD CONSTRAINT editions_pkey PRIMARY KEY (id);

--
-- Name: email_outbox_event_announcements email_outbox_event_announcements_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY email_outbox_event_announcements
    ADD CONSTRAINT email_outbox_event_announcements_pkey PRIMARY KEY (outbox_id);

--
-- Name: email_outbox email_outbox_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY email_outbox
    ADD CONSTRAINT email_outbox_pkey PRIMARY KEY (id);

--
-- Name: email_outbox_reminder_loans email_outbox_reminder_loans_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY email_outbox_reminder_loans
    ADD CONSTRAINT email_outbox_reminder_loans_pkey PRIMARY KEY (loan_id);

--
-- Name: email_templates email_templates_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY email_templates
    ADD CONSTRAINT email_templates_pkey PRIMARY KEY (template_id, language);

--
-- Name: equipment equipment_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY equipment
    ADD CONSTRAINT equipment_pkey PRIMARY KEY (id);

--
-- Name: event_audiences event_audiences_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY event_audiences
    ADD CONSTRAINT event_audiences_pkey PRIMARY KEY (event_id, audience);

--
-- Name: events events_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY events
    ADD CONSTRAINT events_pkey PRIMARY KEY (id);

--
-- Name: fees fees_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY fees
    ADD CONSTRAINT fees_pkey PRIMARY KEY (code);

--
-- Name: fine_rules fine_rules_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY fine_rules
    ADD CONSTRAINT fine_rules_pkey PRIMARY KEY (id);

--
-- Name: fines fines_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY fines
    ADD CONSTRAINT fines_pkey PRIMARY KEY (id);

--
-- Name: genre_labels genre_labels_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY genre_labels
    ADD CONSTRAINT genre_labels_pkey PRIMARY KEY (genre_code, language);

--
-- Name: genre_synonyms genre_synonyms_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY genre_synonyms
    ADD CONSTRAINT genre_synonyms_pkey PRIMARY KEY (id);

--
-- Name: genre_synonyms genre_synonyms_term_kind_uq; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY genre_synonyms
    ADD CONSTRAINT genre_synonyms_term_kind_uq UNIQUE (match_kind, term_normalized);

--
-- Name: genres genres_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY genres
    ADD CONSTRAINT genres_pkey PRIMARY KEY (code);

--
-- Name: holds holds_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY holds
    ADD CONSTRAINT holds_pkey PRIMARY KEY (id);

--
-- Name: idempotency_keys idempotency_keys_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY idempotency_keys
    ADD CONSTRAINT idempotency_keys_pkey PRIMARY KEY (actor_id, idempotency_key);

--
-- Name: inventory_scans inventory_scans_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY inventory_scans
    ADD CONSTRAINT inventory_scans_pkey PRIMARY KEY (id);

--
-- Name: inventory_sessions inventory_sessions_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY inventory_sessions
    ADD CONSTRAINT inventory_sessions_pkey PRIMARY KEY (id);

--
-- Name: item_transits item_transits_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY item_transits
    ADD CONSTRAINT item_transits_pkey PRIMARY KEY (id);

--
-- Name: item_withdrawals item_withdrawals_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY item_withdrawals
    ADD CONSTRAINT item_withdrawals_pkey PRIMARY KEY (id);

--
-- Name: items items_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY items
    ADD CONSTRAINT items_pkey PRIMARY KEY (id);

--
-- Name: lenders lenders_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY lenders
    ADD CONSTRAINT lenders_pkey PRIMARY KEY (id);

--
-- Name: library_info library_info_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY library_info
    ADD CONSTRAINT library_info_pkey PRIMARY KEY (id);

--
-- Name: loans_archives loans_archives_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY loans_archives
    ADD CONSTRAINT loans_archives_pkey PRIMARY KEY (id);

--
-- Name: loans loans_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY loans
    ADD CONSTRAINT loans_pkey PRIMARY KEY (id);

--
-- Name: loans_settings loans_settings_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY loans_settings
    ADD CONSTRAINT loans_settings_pkey PRIMARY KEY (id);

--
-- Name: public_type_loan_settings public_type_loan_settings_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public_type_loan_settings
    ADD CONSTRAINT public_type_loan_settings_pkey PRIMARY KEY (id);

--
-- Name: public_types public_types_name_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public_types
    ADD CONSTRAINT public_types_name_key UNIQUE (name);

--
-- Name: public_types public_types_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public_types
    ADD CONSTRAINT public_types_pkey PRIMARY KEY (id);

--
-- Name: purchase_order_lines purchase_order_lines_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY purchase_order_lines
    ADD CONSTRAINT purchase_order_lines_pkey PRIMARY KEY (id);

--
-- Name: purchase_orders purchase_orders_order_number_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY purchase_orders
    ADD CONSTRAINT purchase_orders_order_number_key UNIQUE (order_number);

--
-- Name: purchase_orders purchase_orders_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY purchase_orders
    ADD CONSTRAINT purchase_orders_pkey PRIMARY KEY (id);

--
-- Name: purchase_suggestions purchase_suggestions_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY purchase_suggestions
    ADD CONSTRAINT purchase_suggestions_pkey PRIMARY KEY (id);

--
-- Name: receipt_items receipt_items_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY receipt_items
    ADD CONSTRAINT receipt_items_pkey PRIMARY KEY (receipt_line_id, item_id);

--
-- Name: receipt_lines receipt_lines_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY receipt_lines
    ADD CONSTRAINT receipt_lines_pkey PRIMARY KEY (id);

--
-- Name: receipts receipts_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY receipts
    ADD CONSTRAINT receipts_pkey PRIMARY KEY (id);

--
-- Name: retired_card_numbers retired_card_numbers_card_number_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY retired_card_numbers
    ADD CONSTRAINT retired_card_numbers_card_number_key UNIQUE (card_number);

--
-- Name: retired_card_numbers retired_card_numbers_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY retired_card_numbers
    ADD CONSTRAINT retired_card_numbers_pkey PRIMARY KEY (id);

--
-- Name: saved_queries saved_queries_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY saved_queries
    ADD CONSTRAINT saved_queries_pkey PRIMARY KEY (id);

--
-- Name: schedule_closures schedule_closures_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY schedule_closures
    ADD CONSTRAINT schedule_closures_pkey PRIMARY KEY (id);

--
-- Name: schedule_periods schedule_periods_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY schedule_periods
    ADD CONSTRAINT schedule_periods_pkey PRIMARY KEY (id);

--
-- Name: schedule_slots schedule_slots_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY schedule_slots
    ADD CONSTRAINT schedule_slots_pkey PRIMARY KEY (id);

--
-- Name: series series_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY series
    ADD CONSTRAINT series_pkey PRIMARY KEY (id);

--
-- Name: settings settings_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY settings
    ADD CONSTRAINT settings_pkey PRIMARY KEY (key);

--
-- Name: sites sites_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY sites
    ADD CONSTRAINT sites_pkey PRIMARY KEY (id);

--
-- Name: user_guardians user_guardians_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY user_guardians
    ADD CONSTRAINT user_guardians_pkey PRIMARY KEY (child_id);

--
-- Name: users users_barcode_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY users
    ADD CONSTRAINT users_barcode_key UNIQUE (barcode);

--
-- Name: users users_login_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY users
    ADD CONSTRAINT users_login_key UNIQUE (login);

--
-- Name: users users_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY users
    ADD CONSTRAINT users_pkey PRIMARY KEY (id);

--
-- Name: vendors vendors_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY vendors
    ADD CONSTRAINT vendors_pkey PRIMARY KEY (id);

--
-- Name: visitor_counts visitor_counts_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY visitor_counts
    ADD CONSTRAINT visitor_counts_pkey PRIMARY KEY (id);

--
-- Name: z3950servers z3950servers_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY z3950servers
    ADD CONSTRAINT z3950servers_pkey PRIMARY KEY (id);

--
-- Name: audit_log_created_at_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX audit_log_created_at_idx ON audit_log USING btree (created_at DESC);

--
-- Name: audit_log_entity_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX audit_log_entity_idx ON audit_log USING btree (entity_type, entity_id);

--
-- Name: audit_log_event_type_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX audit_log_event_type_idx ON audit_log USING btree (event_type);

--
-- Name: audit_log_outcome_created_at_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX audit_log_outcome_created_at_idx ON audit_log USING btree (outcome, created_at DESC);

--
-- Name: audit_log_user_id_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX audit_log_user_id_idx ON audit_log USING btree (user_id);

--
-- Name: email_outbox_event_announcements_event_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX email_outbox_event_announcements_event_idx ON email_outbox_event_announcements USING btree (event_id);

--
-- Name: email_outbox_pending_created_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX email_outbox_pending_created_idx ON email_outbox USING btree (created_at) WHERE ((status)::text = 'pending'::text);

--
-- Name: email_outbox_reminder_loans_outbox_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX email_outbox_reminder_loans_outbox_idx ON email_outbox_reminder_loans USING btree (outbox_id);

--
-- Name: fine_rules_default_key; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX fine_rules_default_key ON fine_rules USING btree ((true)) WHERE (media_type IS NULL);

--
-- Name: fine_rules_media_type_key; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX fine_rules_media_type_key ON fine_rules USING btree (media_type) WHERE (media_type IS NOT NULL);

--
-- Name: idx_acquisition_funds_code_year; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX idx_acquisition_funds_code_year ON acquisition_funds USING btree (lower((code)::text), fiscal_year);

--
-- Name: idx_acquisition_funds_year; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_acquisition_funds_year ON acquisition_funds USING btree (fiscal_year);

--
-- Name: idx_biblio_authors_author; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_biblio_authors_author ON biblio_authors USING btree (author_id);

--
-- Name: idx_biblio_authors_biblio; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_biblio_authors_biblio ON biblio_authors USING btree (biblio_id);

--
-- Name: idx_biblio_collections_biblio; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_biblio_collections_biblio ON biblio_collections USING btree (biblio_id);

--
-- Name: idx_biblio_collections_collection; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_biblio_collections_collection ON biblio_collections USING btree (collection_id);

--
-- Name: idx_biblio_dewey; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_biblio_dewey ON biblios USING btree (dewey) WHERE (dewey IS NOT NULL);

--
-- Name: idx_biblio_series_biblio; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_biblio_series_biblio ON biblio_series USING btree (biblio_id);

--
-- Name: idx_biblio_series_series; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_biblio_series_series ON biblio_series USING btree (series_id);

--
-- Name: idx_biblios_active; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_biblios_active ON biblios USING btree (archived_at) WHERE (archived_at IS NULL);

--
-- Name: idx_biblios_isbn; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_biblios_isbn ON biblios USING btree (isbn) WHERE (isbn IS NOT NULL);

--
-- Name: idx_biblios_media_type; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_biblios_media_type ON biblios USING btree (media_type);

--
-- Name: idx_collections_key_unique; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX idx_collections_key_unique ON collections USING btree (key) WHERE (key IS NOT NULL);

--
-- Name: idx_event_audiences_audience; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_event_audiences_audience ON event_audiences USING btree (audience);

--
-- Name: idx_events_date; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_events_date ON events USING btree (event_date);

--
-- Name: idx_events_type; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_events_type ON events USING btree (event_type);

--
-- Name: idx_fines_item_replacement; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_fines_item_replacement ON fines USING btree (item_id) WHERE (((charge_type)::text = 'replacement'::text) AND (item_id IS NOT NULL));

--
-- Name: idx_fines_loan_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_fines_loan_id ON fines USING btree (loan_id);

--
-- Name: idx_fines_one_open_per_loan_charge; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX idx_fines_one_open_per_loan_charge ON fines USING btree (loan_id, charge_type) WHERE ((status)::text = ANY ((ARRAY['pending'::character varying, 'partial'::character varying])::text[]));

--
-- Name: idx_fines_user_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_fines_user_status ON fines USING btree (user_id, status);

--
-- Name: idx_genre_synonyms_norm; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_genre_synonyms_norm ON genre_synonyms USING btree (term_normalized);

--
-- Name: idx_holds_biblio_queue; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_holds_biblio_queue ON holds USING btree (biblio_id, "position", created_at) WHERE ((status)::text = ANY ((ARRAY['pending'::character varying, 'to_pick'::character varying, 'ready'::character varying])::text[]));

--
-- Name: idx_holds_biblio_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_holds_biblio_status ON holds USING btree (biblio_id, status);

--
-- Name: idx_holds_item_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_holds_item_id ON holds USING btree (item_id);

--
-- Name: idx_holds_item_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_holds_item_status ON holds USING btree (item_id, status);

--
-- Name: idx_holds_one_active_per_user_biblio; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX idx_holds_one_active_per_user_biblio ON holds USING btree (user_id, biblio_id) WHERE ((status)::text = ANY ((ARRAY['pending'::character varying, 'to_pick'::character varying, 'ready'::character varying])::text[]));

--
-- Name: INDEX idx_holds_one_active_per_user_biblio; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON INDEX idx_holds_one_active_per_user_biblio IS 'At most one pending/to_pick/ready hold per user and bibliographic record.';

--
-- Name: idx_holds_one_exclusive_assignment_per_item; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX idx_holds_one_exclusive_assignment_per_item ON holds USING btree (item_id) WHERE ((status)::text = ANY ((ARRAY['to_pick'::character varying, 'ready'::character varying])::text[]));

--
-- Name: INDEX idx_holds_one_exclusive_assignment_per_item; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON INDEX idx_holds_one_exclusive_assignment_per_item IS 'At most one to_pick or ready hold per copy. In-transit exclusivity lives on item_transits, not holds.status.';

--
-- Name: idx_holds_user_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_holds_user_id ON holds USING btree (user_id);

--
-- Name: idx_idempotency_keys_expires_at; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_idempotency_keys_expires_at ON idempotency_keys USING btree (expires_at);

--
-- Name: idx_inventory_scans_scanned_by; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_inventory_scans_scanned_by ON inventory_scans USING btree (scanned_by);

--
-- Name: idx_inventory_scans_session; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_inventory_scans_session ON inventory_scans USING btree (session_id);

--
-- Name: idx_inventory_scans_session_item; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_inventory_scans_session_item ON inventory_scans USING btree (session_id, item_id);

--
-- Name: idx_inventory_sessions_consolidated_at; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_inventory_sessions_consolidated_at ON inventory_sessions USING btree (consolidated_at) WHERE (consolidated_at IS NOT NULL);

--
-- Name: idx_inventory_sessions_open_scope; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX idx_inventory_sessions_open_scope ON inventory_sessions USING btree (COALESCE(scope_site_id, (- (1)::bigint)), COALESCE(scope_place, (- (1)::smallint))) WHERE ((status)::text = 'open'::text);

--
-- Name: idx_inventory_sessions_started_at; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_inventory_sessions_started_at ON inventory_sessions USING btree (started_at DESC);

--
-- Name: idx_inventory_sessions_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_inventory_sessions_status ON inventory_sessions USING btree (status);

--
-- Name: idx_item_transits_hold; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_item_transits_hold ON item_transits USING btree (hold_id);

--
-- Name: idx_item_transits_one_active_per_hold; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX idx_item_transits_one_active_per_hold ON item_transits USING btree (hold_id) WHERE ((hold_id IS NOT NULL) AND ((status)::text = ANY ((ARRAY['requested'::character varying, 'in_transit'::character varying])::text[])));

--
-- Name: idx_item_transits_one_active_per_item; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX idx_item_transits_one_active_per_item ON item_transits USING btree (item_id) WHERE ((status)::text = ANY ((ARRAY['requested'::character varying, 'in_transit'::character varying])::text[]));

--
-- Name: idx_item_transits_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_item_transits_status ON item_transits USING btree (status);

--
-- Name: idx_item_transits_to_source; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_item_transits_to_source ON item_transits USING btree (to_site_id) WHERE ((status)::text = ANY ((ARRAY['requested'::character varying, 'in_transit'::character varying])::text[]));

--
-- Name: idx_item_withdrawals_one_active; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX idx_item_withdrawals_one_active ON item_withdrawals USING btree (item_id) WHERE (cancelled_at IS NULL);

--
-- Name: idx_item_withdrawals_reason; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_item_withdrawals_reason ON item_withdrawals USING btree (reason);

--
-- Name: idx_item_withdrawals_withdrawn_at; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_item_withdrawals_withdrawn_at ON item_withdrawals USING btree (withdrawn_at DESC);

--
-- Name: idx_items_active; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_items_active ON items USING btree (archived_at) WHERE (archived_at IS NULL);

--
-- Name: idx_items_barcode_active_unique; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX idx_items_barcode_active_unique ON items USING btree (barcode) WHERE ((barcode IS NOT NULL) AND (archived_at IS NULL));

--
-- Name: idx_items_biblio; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_items_biblio ON items USING btree (biblio_id);

--
-- Name: idx_items_deposit_batch; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_items_deposit_batch ON items USING btree (deposit_batch_id) WHERE (deposit_batch_id IS NOT NULL);

--
-- Name: idx_items_deposit_lender_barcode; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_items_deposit_lender_barcode ON items USING btree (deposit_batch_id, lender_barcode) WHERE ((deposit_batch_id IS NOT NULL) AND (lender_barcode IS NOT NULL));

--
-- Name: idx_items_site_active; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_items_site_active ON items USING btree (site_id) WHERE (archived_at IS NULL);

--
-- Name: idx_items_weeding_withdrawn; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_items_weeding_withdrawn ON items USING btree (biblio_id) WHERE (((weeding_status)::text = 'withdrawn'::text) AND (archived_at IS NULL));

--
-- Name: idx_loans_active; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_loans_active ON loans USING btree (returned_at) WHERE (returned_at IS NULL);

--
-- Name: idx_loans_archives_age_band; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_loans_archives_age_band ON loans_archives USING btree (age_band);

--
-- Name: idx_loans_archives_item_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_loans_archives_item_id ON loans_archives USING btree (item_id);

--
-- Name: idx_loans_archives_loan_year; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_loans_archives_loan_year ON loans_archives USING btree (loan_year);

--
-- Name: idx_loans_archives_site_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_loans_archives_site_id ON loans_archives USING btree (site_id);

--
-- Name: idx_loans_archives_user; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_loans_archives_user ON loans_archives USING btree (user_id);

--
-- Name: idx_loans_item_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_loans_item_id ON loans USING btree (item_id);

--
-- Name: idx_loans_one_active_per_item; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX idx_loans_one_active_per_item ON loans USING btree (item_id) WHERE (returned_at IS NULL);

--
-- Name: INDEX idx_loans_one_active_per_item; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON INDEX idx_loans_one_active_per_item IS 'At most one unreturned loan per item (physical copy).';

--
-- Name: idx_loans_site_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_loans_site_id ON loans USING btree (site_id);

--
-- Name: idx_loans_user_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_loans_user_id ON loans USING btree (user_id);

--
-- Name: idx_po_lines_biblio; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_po_lines_biblio ON purchase_order_lines USING btree (biblio_id);

--
-- Name: idx_po_lines_fund; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_po_lines_fund ON purchase_order_lines USING btree (fund_id);

--
-- Name: idx_po_lines_order; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_po_lines_order ON purchase_order_lines USING btree (purchase_order_id);

--
-- Name: idx_public_type_loan_settings_public_type; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_public_type_loan_settings_public_type ON public_type_loan_settings USING btree (public_type_id);

--
-- Name: idx_purchase_orders_fund; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_purchase_orders_fund ON purchase_orders USING btree (fund_id);

--
-- Name: idx_purchase_orders_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_purchase_orders_status ON purchase_orders USING btree (status);

--
-- Name: idx_purchase_orders_vendor; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_purchase_orders_vendor ON purchase_orders USING btree (vendor_id);

--
-- Name: idx_purchase_suggestions_created; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_purchase_suggestions_created ON purchase_suggestions USING btree (created_at DESC);

--
-- Name: idx_purchase_suggestions_proposed_by; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_purchase_suggestions_proposed_by ON purchase_suggestions USING btree (proposed_by);

--
-- Name: idx_purchase_suggestions_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_purchase_suggestions_status ON purchase_suggestions USING btree (status);

--
-- Name: idx_receipt_items_item; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_receipt_items_item ON receipt_items USING btree (item_id);

--
-- Name: idx_receipt_lines_po_line; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_receipt_lines_po_line ON receipt_lines USING btree (purchase_order_line_id);

--
-- Name: idx_receipt_lines_receipt; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_receipt_lines_receipt ON receipt_lines USING btree (receipt_id);

--
-- Name: idx_receipts_order; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_receipts_order ON receipts USING btree (purchase_order_id);

--
-- Name: idx_retired_card_numbers_user_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_retired_card_numbers_user_id ON retired_card_numbers USING btree (user_id);

--
-- Name: idx_saved_queries_shared; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_saved_queries_shared ON saved_queries USING btree (is_shared) WHERE (is_shared = true);

--
-- Name: idx_saved_queries_user_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_saved_queries_user_id ON saved_queries USING btree (user_id);

--
-- Name: idx_schedule_closures_date; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_schedule_closures_date ON schedule_closures USING btree (closure_date);

--
-- Name: idx_schedule_closures_source_date; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_schedule_closures_source_date ON schedule_closures USING btree (site_id, closure_date);

--
-- Name: idx_schedule_slots_period; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_schedule_slots_period ON schedule_slots USING btree (period_id);

--
-- Name: idx_series_key_unique; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX idx_series_key_unique ON series USING btree (key) WHERE (key IS NOT NULL);

--
-- Name: idx_user_guardians_guardian; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_user_guardians_guardian ON user_guardians USING btree (guardian_id);

--
-- Name: idx_users_account_type; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_users_account_type ON users USING btree (account_type);

--
-- Name: idx_users_public_type; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_users_public_type ON users USING btree (public_type);

--
-- Name: idx_vendors_code_active; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX idx_vendors_code_active ON vendors USING btree (lower((code)::text)) WHERE ((code IS NOT NULL) AND (archived_at IS NULL));

--
-- Name: idx_vendors_name; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_vendors_name ON vendors USING btree (lower((name)::text));

--
-- Name: idx_visitor_counts_date; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_visitor_counts_date ON visitor_counts USING btree (count_date);

--
-- Name: lenders_name_active_uniq; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX lenders_name_active_uniq ON lenders USING btree (lower(name)) WHERE (archived_at IS NULL);

--
-- Name: loans_settings_default_row; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX loans_settings_default_row ON loans_settings USING btree ((1)) WHERE (media_type IS NULL);

--
-- Name: loans_settings_media_type_unique; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX loans_settings_media_type_unique ON loans_settings USING btree (media_type) WHERE (media_type IS NOT NULL);

--
-- Name: ptls_public_default_row; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX ptls_public_default_row ON public_type_loan_settings USING btree (public_type_id) WHERE (media_type IS NULL);

--
-- Name: ptls_public_media_unique; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX ptls_public_media_unique ON public_type_loan_settings USING btree (public_type_id, media_type) WHERE (media_type IS NOT NULL);

--
-- Name: sites_default_unique; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX sites_default_unique ON sites USING btree ("default") WHERE ("default" = true);

--
-- Name: biblio_authors biblio_authors_author_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY biblio_authors
    ADD CONSTRAINT biblio_authors_author_id_fkey FOREIGN KEY (author_id) REFERENCES authors(id) ON DELETE CASCADE;

--
-- Name: biblio_authors biblio_authors_biblio_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY biblio_authors
    ADD CONSTRAINT biblio_authors_biblio_id_fkey FOREIGN KEY (biblio_id) REFERENCES biblios(id) ON DELETE CASCADE;

--
-- Name: biblio_collections biblio_collections_biblio_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY biblio_collections
    ADD CONSTRAINT biblio_collections_biblio_id_fkey FOREIGN KEY (biblio_id) REFERENCES biblios(id) ON DELETE CASCADE;

--
-- Name: biblio_collections biblio_collections_collection_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY biblio_collections
    ADD CONSTRAINT biblio_collections_collection_id_fkey FOREIGN KEY (collection_id) REFERENCES collections(id) ON DELETE CASCADE;

--
-- Name: biblio_series biblio_series_biblio_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY biblio_series
    ADD CONSTRAINT biblio_series_biblio_id_fkey FOREIGN KEY (biblio_id) REFERENCES biblios(id) ON DELETE CASCADE;

--
-- Name: biblio_series biblio_series_series_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY biblio_series
    ADD CONSTRAINT biblio_series_series_id_fkey FOREIGN KEY (series_id) REFERENCES series(id) ON DELETE CASCADE;

--
-- Name: biblios biblios_edition_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY biblios
    ADD CONSTRAINT biblios_edition_id_fkey FOREIGN KEY (edition_id) REFERENCES editions(id) ON DELETE SET NULL;

--
-- Name: biblios biblios_source_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY biblios
    ADD CONSTRAINT biblios_source_id_fkey FOREIGN KEY (site_id) REFERENCES sites(id) ON DELETE SET NULL;

--
-- Name: deposit_batches deposit_batches_lender_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY deposit_batches
    ADD CONSTRAINT deposit_batches_lender_id_fkey FOREIGN KEY (lender_id) REFERENCES lenders(id);

--
-- Name: deposit_batches deposit_batches_site_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY deposit_batches
    ADD CONSTRAINT deposit_batches_site_id_fkey FOREIGN KEY (site_id) REFERENCES sites(id);

--
-- Name: email_outbox_event_announcements email_outbox_event_announcements_event_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY email_outbox_event_announcements
    ADD CONSTRAINT email_outbox_event_announcements_event_id_fkey FOREIGN KEY (event_id) REFERENCES events(id) ON DELETE CASCADE;

--
-- Name: email_outbox_event_announcements email_outbox_event_announcements_outbox_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY email_outbox_event_announcements
    ADD CONSTRAINT email_outbox_event_announcements_outbox_id_fkey FOREIGN KEY (outbox_id) REFERENCES email_outbox(id) ON DELETE CASCADE;

--
-- Name: email_outbox_reminder_loans email_outbox_reminder_loans_loan_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY email_outbox_reminder_loans
    ADD CONSTRAINT email_outbox_reminder_loans_loan_id_fkey FOREIGN KEY (loan_id) REFERENCES loans(id) ON DELETE CASCADE;

--
-- Name: email_outbox_reminder_loans email_outbox_reminder_loans_outbox_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY email_outbox_reminder_loans
    ADD CONSTRAINT email_outbox_reminder_loans_outbox_id_fkey FOREIGN KEY (outbox_id) REFERENCES email_outbox(id) ON DELETE CASCADE;

--
-- Name: event_audiences event_audiences_audience_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY event_audiences
    ADD CONSTRAINT event_audiences_audience_fkey FOREIGN KEY (audience) REFERENCES public_types(name) ON UPDATE CASCADE;

--
-- Name: event_audiences event_audiences_event_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY event_audiences
    ADD CONSTRAINT event_audiences_event_id_fkey FOREIGN KEY (event_id) REFERENCES events(id) ON DELETE CASCADE;

--
-- Name: events events_public_type_name_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY events
    ADD CONSTRAINT events_public_type_name_fkey FOREIGN KEY (public_type) REFERENCES public_types(name) ON DELETE SET NULL;

--
-- Name: fines fines_item_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY fines
    ADD CONSTRAINT fines_item_id_fkey FOREIGN KEY (item_id) REFERENCES items(id) ON DELETE SET NULL;

--
-- Name: fines fines_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY fines
    ADD CONSTRAINT fines_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE SET NULL;

--
-- Name: genre_labels genre_labels_genre_code_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY genre_labels
    ADD CONSTRAINT genre_labels_genre_code_fkey FOREIGN KEY (genre_code) REFERENCES genres(code) ON DELETE CASCADE;

--
-- Name: genre_synonyms genre_synonyms_genre_code_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY genre_synonyms
    ADD CONSTRAINT genre_synonyms_genre_code_fkey FOREIGN KEY (genre_code) REFERENCES genres(code) ON DELETE CASCADE;

--
-- Name: holds holds_biblio_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY holds
    ADD CONSTRAINT holds_biblio_id_fkey FOREIGN KEY (biblio_id) REFERENCES biblios(id) ON DELETE CASCADE;

--
-- Name: holds holds_item_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY holds
    ADD CONSTRAINT holds_item_id_fkey FOREIGN KEY (item_id) REFERENCES items(id) ON DELETE CASCADE;

--
-- Name: holds holds_pickup_site_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY holds
    ADD CONSTRAINT holds_pickup_site_id_fkey FOREIGN KEY (pickup_site_id) REFERENCES sites(id) ON DELETE SET NULL;

--
-- Name: holds holds_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY holds
    ADD CONSTRAINT holds_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE;

--
-- Name: inventory_scans inventory_scans_item_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY inventory_scans
    ADD CONSTRAINT inventory_scans_item_id_fkey FOREIGN KEY (item_id) REFERENCES items(id) ON DELETE SET NULL;

--
-- Name: inventory_scans inventory_scans_scanned_by_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY inventory_scans
    ADD CONSTRAINT inventory_scans_scanned_by_fkey FOREIGN KEY (scanned_by) REFERENCES users(id) ON DELETE SET NULL;

--
-- Name: inventory_scans inventory_scans_session_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY inventory_scans
    ADD CONSTRAINT inventory_scans_session_id_fkey FOREIGN KEY (session_id) REFERENCES inventory_sessions(id) ON DELETE CASCADE;

--
-- Name: inventory_sessions inventory_sessions_consolidated_by_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY inventory_sessions
    ADD CONSTRAINT inventory_sessions_consolidated_by_fkey FOREIGN KEY (consolidated_by) REFERENCES users(id) ON DELETE SET NULL;

--
-- Name: inventory_sessions inventory_sessions_created_by_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY inventory_sessions
    ADD CONSTRAINT inventory_sessions_created_by_fkey FOREIGN KEY (created_by) REFERENCES users(id) ON DELETE SET NULL;

--
-- Name: inventory_sessions inventory_sessions_scope_source_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY inventory_sessions
    ADD CONSTRAINT inventory_sessions_scope_source_id_fkey FOREIGN KEY (scope_site_id) REFERENCES sites(id) ON DELETE SET NULL;

--
-- Name: item_transits item_transits_cancelled_by_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY item_transits
    ADD CONSTRAINT item_transits_cancelled_by_fkey FOREIGN KEY (cancelled_by) REFERENCES users(id) ON DELETE SET NULL;

--
-- Name: item_transits item_transits_from_source_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY item_transits
    ADD CONSTRAINT item_transits_from_source_id_fkey FOREIGN KEY (from_site_id) REFERENCES sites(id);

--
-- Name: item_transits item_transits_hold_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY item_transits
    ADD CONSTRAINT item_transits_hold_id_fkey FOREIGN KEY (hold_id) REFERENCES holds(id) ON DELETE SET NULL;

--
-- Name: item_transits item_transits_item_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY item_transits
    ADD CONSTRAINT item_transits_item_id_fkey FOREIGN KEY (item_id) REFERENCES items(id) ON DELETE CASCADE;

--
-- Name: item_transits item_transits_received_by_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY item_transits
    ADD CONSTRAINT item_transits_received_by_fkey FOREIGN KEY (received_by) REFERENCES users(id) ON DELETE SET NULL;

--
-- Name: item_transits item_transits_reversed_from_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY item_transits
    ADD CONSTRAINT item_transits_reversed_from_id_fkey FOREIGN KEY (reversed_from_id) REFERENCES item_transits(id) ON DELETE SET NULL;

--
-- Name: item_transits item_transits_shipped_by_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY item_transits
    ADD CONSTRAINT item_transits_shipped_by_fkey FOREIGN KEY (shipped_by) REFERENCES users(id) ON DELETE SET NULL;

--
-- Name: item_transits item_transits_to_source_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY item_transits
    ADD CONSTRAINT item_transits_to_source_id_fkey FOREIGN KEY (to_site_id) REFERENCES sites(id);

--
-- Name: item_withdrawals item_withdrawals_agent_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY item_withdrawals
    ADD CONSTRAINT item_withdrawals_agent_id_fkey FOREIGN KEY (agent_id) REFERENCES users(id) ON DELETE SET NULL;

--
-- Name: item_withdrawals item_withdrawals_cancelled_by_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY item_withdrawals
    ADD CONSTRAINT item_withdrawals_cancelled_by_fkey FOREIGN KEY (cancelled_by) REFERENCES users(id) ON DELETE SET NULL;

--
-- Name: item_withdrawals item_withdrawals_disposed_by_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY item_withdrawals
    ADD CONSTRAINT item_withdrawals_disposed_by_fkey FOREIGN KEY (disposed_by) REFERENCES users(id) ON DELETE SET NULL;

--
-- Name: item_withdrawals item_withdrawals_item_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY item_withdrawals
    ADD CONSTRAINT item_withdrawals_item_id_fkey FOREIGN KEY (item_id) REFERENCES items(id) ON DELETE RESTRICT;

--
-- Name: items items_biblio_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY items
    ADD CONSTRAINT items_biblio_id_fkey FOREIGN KEY (biblio_id) REFERENCES biblios(id) ON DELETE CASCADE;

--
-- Name: items items_deposit_batch_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY items
    ADD CONSTRAINT items_deposit_batch_id_fkey FOREIGN KEY (deposit_batch_id) REFERENCES deposit_batches(id);

--
-- Name: items items_source_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY items
    ADD CONSTRAINT items_source_id_fkey FOREIGN KEY (site_id) REFERENCES sites(id) ON DELETE SET NULL;

--
-- Name: loans_archives loans_archives_borrower_public_type_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY loans_archives
    ADD CONSTRAINT loans_archives_borrower_public_type_fkey FOREIGN KEY (borrower_public_type) REFERENCES public_types(id) ON DELETE SET NULL;

--
-- Name: loans_archives loans_archives_item_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY loans_archives
    ADD CONSTRAINT loans_archives_item_id_fkey FOREIGN KEY (item_id) REFERENCES items(id) ON DELETE SET NULL;

--
-- Name: loans_archives loans_archives_site_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY loans_archives
    ADD CONSTRAINT loans_archives_site_id_fkey FOREIGN KEY (site_id) REFERENCES sites(id) ON DELETE RESTRICT;

--
-- Name: loans loans_item_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY loans
    ADD CONSTRAINT loans_item_id_fkey FOREIGN KEY (item_id) REFERENCES items(id);

--
-- Name: loans loans_site_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY loans
    ADD CONSTRAINT loans_site_id_fkey FOREIGN KEY (site_id) REFERENCES sites(id) ON DELETE SET NULL;

--
-- Name: public_type_loan_settings public_type_loan_settings_public_type_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public_type_loan_settings
    ADD CONSTRAINT public_type_loan_settings_public_type_id_fkey FOREIGN KEY (public_type_id) REFERENCES public_types(id) ON DELETE CASCADE;

--
-- Name: purchase_order_lines purchase_order_lines_biblio_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY purchase_order_lines
    ADD CONSTRAINT purchase_order_lines_biblio_id_fkey FOREIGN KEY (biblio_id) REFERENCES biblios(id) ON DELETE SET NULL;

--
-- Name: purchase_order_lines purchase_order_lines_fund_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY purchase_order_lines
    ADD CONSTRAINT purchase_order_lines_fund_id_fkey FOREIGN KEY (fund_id) REFERENCES acquisition_funds(id) ON DELETE SET NULL;

--
-- Name: purchase_order_lines purchase_order_lines_purchase_order_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY purchase_order_lines
    ADD CONSTRAINT purchase_order_lines_purchase_order_id_fkey FOREIGN KEY (purchase_order_id) REFERENCES purchase_orders(id) ON DELETE CASCADE;

--
-- Name: purchase_orders purchase_orders_created_by_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY purchase_orders
    ADD CONSTRAINT purchase_orders_created_by_fkey FOREIGN KEY (created_by) REFERENCES users(id) ON DELETE SET NULL;

--
-- Name: purchase_orders purchase_orders_fund_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY purchase_orders
    ADD CONSTRAINT purchase_orders_fund_id_fkey FOREIGN KEY (fund_id) REFERENCES acquisition_funds(id) ON DELETE SET NULL;

--
-- Name: purchase_orders purchase_orders_vendor_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY purchase_orders
    ADD CONSTRAINT purchase_orders_vendor_id_fkey FOREIGN KEY (vendor_id) REFERENCES vendors(id);

--
-- Name: purchase_suggestions purchase_suggestions_proposed_by_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY purchase_suggestions
    ADD CONSTRAINT purchase_suggestions_proposed_by_fkey FOREIGN KEY (proposed_by) REFERENCES users(id) ON DELETE CASCADE;

--
-- Name: purchase_suggestions purchase_suggestions_purchase_order_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY purchase_suggestions
    ADD CONSTRAINT purchase_suggestions_purchase_order_id_fkey FOREIGN KEY (purchase_order_id) REFERENCES purchase_orders(id) ON DELETE SET NULL;

--
-- Name: purchase_suggestions purchase_suggestions_purchase_order_line_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY purchase_suggestions
    ADD CONSTRAINT purchase_suggestions_purchase_order_line_id_fkey FOREIGN KEY (purchase_order_line_id) REFERENCES purchase_order_lines(id) ON DELETE SET NULL;

--
-- Name: purchase_suggestions purchase_suggestions_reviewed_by_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY purchase_suggestions
    ADD CONSTRAINT purchase_suggestions_reviewed_by_fkey FOREIGN KEY (reviewed_by) REFERENCES users(id) ON DELETE SET NULL;

--
-- Name: receipt_items receipt_items_item_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY receipt_items
    ADD CONSTRAINT receipt_items_item_id_fkey FOREIGN KEY (item_id) REFERENCES items(id) ON DELETE CASCADE;

--
-- Name: receipt_items receipt_items_receipt_line_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY receipt_items
    ADD CONSTRAINT receipt_items_receipt_line_id_fkey FOREIGN KEY (receipt_line_id) REFERENCES receipt_lines(id) ON DELETE CASCADE;

--
-- Name: receipt_lines receipt_lines_purchase_order_line_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY receipt_lines
    ADD CONSTRAINT receipt_lines_purchase_order_line_id_fkey FOREIGN KEY (purchase_order_line_id) REFERENCES purchase_order_lines(id);

--
-- Name: receipt_lines receipt_lines_receipt_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY receipt_lines
    ADD CONSTRAINT receipt_lines_receipt_id_fkey FOREIGN KEY (receipt_id) REFERENCES receipts(id) ON DELETE CASCADE;

--
-- Name: receipts receipts_purchase_order_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY receipts
    ADD CONSTRAINT receipts_purchase_order_id_fkey FOREIGN KEY (purchase_order_id) REFERENCES purchase_orders(id) ON DELETE CASCADE;

--
-- Name: receipts receipts_received_by_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY receipts
    ADD CONSTRAINT receipts_received_by_fkey FOREIGN KEY (received_by) REFERENCES users(id) ON DELETE SET NULL;

--
-- Name: retired_card_numbers retired_card_numbers_retired_by_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY retired_card_numbers
    ADD CONSTRAINT retired_card_numbers_retired_by_fkey FOREIGN KEY (retired_by) REFERENCES users(id);

--
-- Name: retired_card_numbers retired_card_numbers_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY retired_card_numbers
    ADD CONSTRAINT retired_card_numbers_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id);

--
-- Name: saved_queries saved_queries_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY saved_queries
    ADD CONSTRAINT saved_queries_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE;

--
-- Name: schedule_closures schedule_closures_source_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY schedule_closures
    ADD CONSTRAINT schedule_closures_source_id_fkey FOREIGN KEY (site_id) REFERENCES sites(id) ON DELETE CASCADE;

--
-- Name: schedule_slots schedule_slots_period_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY schedule_slots
    ADD CONSTRAINT schedule_slots_period_id_fkey FOREIGN KEY (period_id) REFERENCES schedule_periods(id) ON DELETE CASCADE;

--
-- Name: user_guardians user_guardians_child_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY user_guardians
    ADD CONSTRAINT user_guardians_child_id_fkey FOREIGN KEY (child_id) REFERENCES users(id) ON DELETE CASCADE;

--
-- Name: user_guardians user_guardians_guardian_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY user_guardians
    ADD CONSTRAINT user_guardians_guardian_id_fkey FOREIGN KEY (guardian_id) REFERENCES users(id) ON DELETE RESTRICT;

--
-- Name: users users_account_type_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY users
    ADD CONSTRAINT users_account_type_fkey FOREIGN KEY (account_type) REFERENCES account_types(code) ON DELETE RESTRICT;

--
-- Name: users users_public_type_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY users
    ADD CONSTRAINT users_public_type_fkey FOREIGN KEY (public_type) REFERENCES public_types(id);

-- =============================================================================
-- SEED DATA (idempotent)
-- =============================================================================

DO $seed_account_types$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM account_types LIMIT 1) THEN
        INSERT INTO account_types (code, name, items_rights, users_rights, loans_rights, items_archive_rights, holds_rights, settings_rights, events_rights, acquisitions_rights, weeding_rights, weeding_cancel_rights, is_system, circulation_override_rights) VALUES ('guest', 'Guest', 'r', 'n', 'n', 'n', 'n', 'r', 'r', 'n', 'n', 'n', true, 'n');
        INSERT INTO account_types (code, name, items_rights, users_rights, loans_rights, items_archive_rights, holds_rights, settings_rights, events_rights, acquisitions_rights, weeding_rights, weeding_cancel_rights, is_system, circulation_override_rights) VALUES ('reader', 'Reader', 'r', 'o', 'o', 'r', 'o', 'r', 'r', 'n', 'n', 'n', true, 'n');
        INSERT INTO account_types (code, name, items_rights, users_rights, loans_rights, items_archive_rights, holds_rights, settings_rights, events_rights, acquisitions_rights, weeding_rights, weeding_cancel_rights, is_system, circulation_override_rights) VALUES ('librarian', 'Librarian', 'w', 'w', 'w', 'w', 'w', 'r', 'w', 'w', 'w', 'w', true, 'n');
        INSERT INTO account_types (code, name, items_rights, users_rights, loans_rights, items_archive_rights, holds_rights, settings_rights, events_rights, acquisitions_rights, weeding_rights, weeding_cancel_rights, is_system, circulation_override_rights) VALUES ('group', 'Group', 'r', 'o', 'o', 'r', 'o', 'r', 'r', 'n', 'n', 'n', true, 'n');
        INSERT INTO account_types (code, name, items_rights, users_rights, loans_rights, items_archive_rights, holds_rights, settings_rights, events_rights, acquisitions_rights, weeding_rights, weeding_cancel_rights, is_system, circulation_override_rights) VALUES ('admin', 'Administrator', 'w', 'w', 'w', 'w', 'w', 'w', 'w', 'w', 'w', 'w', true, 'w');
    END IF;
END
$seed_account_types$;

DO $seed_fees$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM fees LIMIT 1) THEN
        INSERT INTO fees (code, name, amount) VALUES ('free', 'Free', 0);
        INSERT INTO fees (code, name, amount) VALUES ('local', 'Local', 0);
        INSERT INTO fees (code, name, amount) VALUES ('foreigner', 'Foreigner', 0);
    END IF;
END
$seed_fees$;

DO $seed_public_types$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM public_types LIMIT 1) THEN
        INSERT INTO public_types (id, name, label, subscription_duration_days, age_min, age_max, subscription_price, unpaid_fine_threshold, max_active_holds, unpaid_threshold_mode) VALUES (1, 'child', 'Child', 365, 0, 12, 0, NULL, NULL, 'inherit');
        INSERT INTO public_types (id, name, label, subscription_duration_days, age_min, age_max, subscription_price, unpaid_fine_threshold, max_active_holds, unpaid_threshold_mode) VALUES (2, 'adult', 'Adult', 365, 18, 99, 1500, NULL, NULL, 'inherit');
        INSERT INTO public_types (id, name, label, subscription_duration_days, age_min, age_max, subscription_price, unpaid_fine_threshold, max_active_holds, unpaid_threshold_mode) VALUES (3, 'school', 'School', 365, NULL, NULL, 0, NULL, NULL, 'inherit');
        INSERT INTO public_types (id, name, label, subscription_duration_days, age_min, age_max, subscription_price, unpaid_fine_threshold, max_active_holds, unpaid_threshold_mode) VALUES (4, 'staff', 'Staff', 365, NULL, NULL, NULL, NULL, NULL, 'inherit');
        INSERT INTO public_types (id, name, label, subscription_duration_days, age_min, age_max, subscription_price, unpaid_fine_threshold, max_active_holds, unpaid_threshold_mode) VALUES (5, 'senior', 'Senior', 365, 60, 99, 0, NULL, NULL, 'inherit');
    END IF;
END
$seed_public_types$;

DO $seed_public_type_loan_settings$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM public_type_loan_settings LIMIT 1) THEN
        INSERT INTO public_type_loan_settings (id, public_type_id, media_type, duration, nb_max, nb_renews, renew_at) VALUES (1, 1, NULL, 21, 10, 2, 'at_due_date');
        INSERT INTO public_type_loan_settings (id, public_type_id, media_type, duration, nb_max, nb_renews, renew_at) VALUES (2, 2, NULL, 21, 5, 2, 'at_due_date');
        INSERT INTO public_type_loan_settings (id, public_type_id, media_type, duration, nb_max, nb_renews, renew_at) VALUES (3, 3, NULL, 60, 50, 2, 'at_due_date');
        INSERT INTO public_type_loan_settings (id, public_type_id, media_type, duration, nb_max, nb_renews, renew_at) VALUES (4, 4, NULL, 21, 5, 2, 'at_due_date');
        INSERT INTO public_type_loan_settings (id, public_type_id, media_type, duration, nb_max, nb_renews, renew_at) VALUES (5, 5, NULL, 21, 5, 2, 'at_due_date');
    END IF;
END
$seed_public_type_loan_settings$;

DO $seed_loans_settings$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM loans_settings LIMIT 1) THEN
        INSERT INTO loans_settings (id, media_type, nb_max, nb_renews, duration, notes, renew_at, renew_window_days) VALUES (1, 'audio', 2, 2, 14, '', 'at_due_date', 7);
        INSERT INTO loans_settings (id, media_type, nb_max, nb_renews, duration, notes, renew_at, renew_window_days) VALUES (2, 'printedText', 3, 2, 21, '', 'at_due_date', 7);
        INSERT INTO loans_settings (id, media_type, nb_max, nb_renews, duration, notes, renew_at, renew_window_days) VALUES (3, 'periodic', 2, 2, 14, '', 'at_due_date', 7);
        INSERT INTO loans_settings (id, media_type, nb_max, nb_renews, duration, notes, renew_at, renew_window_days) VALUES (4, NULL, 5, 2, 21, '', 'at_due_date', 7);
    END IF;
END
$seed_loans_settings$;

DO $seed_circulation_settings$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM circulation_settings LIMIT 1) THEN
        INSERT INTO circulation_settings (id, unpaid_fine_threshold, max_active_holds, pick_delay_days, unpaid_threshold_mode) VALUES (1, 0, 20, 2, 'none');
    END IF;
END
$seed_circulation_settings$;

DO $seed_fine_rules$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM fine_rules LIMIT 1) THEN
        INSERT INTO fine_rules (id, media_type, daily_rate, max_amount, grace_days, notes) VALUES (1, NULL, 20, 1500, 3, 'Default overdue fine rule');
    END IF;
END
$seed_fine_rules$;

DO $seed_sites$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM sites LIMIT 1) THEN
        INSERT INTO sites (id, key, name, is_archive, archived_at, "default") VALUES (1, NULL, 'MyLibrary', 0, NULL, true);
    END IF;
END
$seed_sites$;

DO $seed_library_info$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM library_info LIMIT 1) THEN
        INSERT INTO library_info (id, name, addr_line1, addr_line2, addr_postcode, addr_city, addr_country, phones, email, updated_at, timezone, language) VALUES (1, NULL, NULL, NULL, NULL, NULL, NULL, '[]', NULL, '2026-10-23 12:49:19.817996+00', 'Europe/Paris', NULL);
    END IF;
END
$seed_library_info$;

DO $seed_z3950servers$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM z3950servers LIMIT 1) THEN
        INSERT INTO z3950servers (id, address, port, name, description, activated, login, password, database, format, encoding) VALUES (1, 'z3950.bnf.fr', 2211, 'BNF', 'Bibliothèque nationale de France', true, 'Z3950', 'Z3950_BNF', 'TOUT-UTF8', 'UNIMARC', 'utf-8');
        INSERT INTO z3950servers (id, address, port, name, description, activated, login, password, database, format, encoding) VALUES (2, 'z3950.loc.gov', 7090, 'Library of Congress', 'Library of Congress Z39.50 Server', true, '', '', 'VOYAGER', 'MARC21', 'utf-8');
        INSERT INTO z3950servers (id, address, port, name, description, activated, login, password, database, format, encoding) VALUES (3, 'opac.sudoc.abes.fr', 2200, 'SUDOC', 'Système Universitaire de Documentation', true, '', '', 'abes', 'UNIMARC', 'utf-8');
    END IF;
END
$seed_z3950servers$;

DO $seed_genres$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM genres LIMIT 1) THEN
        INSERT INTO genres (code, shelf_code, active, sort_order) VALUES (0, NULL, false, 0);
        INSERT INTO genres (code, shelf_code, active, sort_order) VALUES (1, 'GEN', true, 10);
        INSERT INTO genres (code, shelf_code, active, sort_order) VALUES (2, 'ROM', true, 20);
        INSERT INTO genres (code, shelf_code, active, sort_order) VALUES (3, 'BD', true, 30);
        INSERT INTO genres (code, shelf_code, active, sort_order) VALUES (4, 'THE', true, 40);
        INSERT INTO genres (code, shelf_code, active, sort_order) VALUES (5, 'POE', true, 50);
        INSERT INTO genres (code, shelf_code, active, sort_order) VALUES (6, 'PHI', true, 60);
        INSERT INTO genres (code, shelf_code, active, sort_order) VALUES (7, 'REL', true, 70);
        INSERT INTO genres (code, shelf_code, active, sort_order) VALUES (8, 'SOC', true, 80);
        INSERT INTO genres (code, shelf_code, active, sort_order) VALUES (9, 'LAN', true, 90);
        INSERT INTO genres (code, shelf_code, active, sort_order) VALUES (10, 'SCI', true, 100);
        INSERT INTO genres (code, shelf_code, active, sort_order) VALUES (11, 'TEC', true, 110);
        INSERT INTO genres (code, shelf_code, active, sort_order) VALUES (12, 'ART', true, 120);
        INSERT INTO genres (code, shelf_code, active, sort_order) VALUES (13, 'SPO', true, 130);
        INSERT INTO genres (code, shelf_code, active, sort_order) VALUES (14, 'LIT', true, 140);
        INSERT INTO genres (code, shelf_code, active, sort_order) VALUES (15, 'HIS', true, 150);
        INSERT INTO genres (code, shelf_code, active, sort_order) VALUES (16, 'GEO', true, 160);
        INSERT INTO genres (code, shelf_code, active, sort_order) VALUES (17, 'AUT', true, 170);
        INSERT INTO genres (code, shelf_code, active, sort_order) VALUES (100, NULL, true, 200);
        INSERT INTO genres (code, shelf_code, active, sort_order) VALUES (101, 'JAZ', true, 210);
        INSERT INTO genres (code, shelf_code, active, sort_order) VALUES (102, 'BLU', true, 220);
        INSERT INTO genres (code, shelf_code, active, sort_order) VALUES (103, 'ROC', true, 230);
        INSERT INTO genres (code, shelf_code, active, sort_order) VALUES (104, 'WLD', true, 240);
        INSERT INTO genres (code, shelf_code, active, sort_order) VALUES (105, 'CLA', true, 250);
        INSERT INTO genres (code, shelf_code, active, sort_order) VALUES (200, NULL, true, 300);
        INSERT INTO genres (code, shelf_code, active, sort_order) VALUES (201, 'VFI', true, 310);
        INSERT INTO genres (code, shelf_code, active, sort_order) VALUES (202, 'VHI', true, 320);
        INSERT INTO genres (code, shelf_code, active, sort_order) VALUES (203, 'VAR', true, 330);
        INSERT INTO genres (code, shelf_code, active, sort_order) VALUES (204, 'DOC', true, 340);
        INSERT INTO genres (code, shelf_code, active, sort_order) VALUES (205, 'MUS', true, 350);
    END IF;
END
$seed_genres$;

DO $seed_genre_labels$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM genre_labels LIMIT 1) THEN
        INSERT INTO genre_labels (genre_code, language, label) VALUES (0, 'fr', 'non spécifié');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (0, 'en', 'Not specified');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (0, 'de', 'Nicht angegeben');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (0, 'es', 'No especificado');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (1, 'fr', 'Généralités');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (1, 'en', 'General');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (1, 'de', 'Allgemeines');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (1, 'es', 'Generalidades');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (2, 'fr', 'Romans');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (2, 'en', 'Fiction');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (2, 'de', 'Fiktion');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (2, 'es', 'Ficción');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (3, 'fr', 'Bandes dessinées');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (3, 'en', 'Comics');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (3, 'de', 'Comic');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (3, 'es', 'Cómic');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (4, 'fr', 'Théâtre');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (4, 'en', 'Theatre');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (4, 'de', 'Theater');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (4, 'es', 'Teatro');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (5, 'fr', 'Poésie');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (5, 'en', 'Poetry');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (5, 'de', 'Poesie');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (5, 'es', 'Poesía');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (6, 'fr', 'Philosophie');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (6, 'en', 'Philosophy');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (6, 'de', 'Philosophie');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (6, 'es', 'Filosofía');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (7, 'fr', 'Religion');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (7, 'en', 'Religion');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (7, 'de', 'Religion');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (7, 'es', 'Religión');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (8, 'fr', 'Sciences sociales');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (8, 'en', 'Social sciences');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (8, 'de', 'Sozialwissenschaften');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (8, 'es', 'Ciencias sociales');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (9, 'fr', 'Langues');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (9, 'en', 'Languages');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (9, 'de', 'Sprachen');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (9, 'es', 'Idiomas');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (10, 'fr', 'Sciences');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (10, 'en', 'Sciences');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (10, 'de', 'Wissenschaften');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (10, 'es', 'Ciencias');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (11, 'fr', 'Technique');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (11, 'en', 'Technical');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (11, 'de', 'Technik');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (11, 'es', 'Técnica');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (12, 'fr', 'Art');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (12, 'en', 'Art');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (12, 'de', 'Kunst');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (12, 'es', 'Arte');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (13, 'fr', 'Sport');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (13, 'en', 'Sport');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (13, 'de', 'Sport');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (13, 'es', 'Deporte');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (14, 'fr', 'Littérature');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (14, 'en', 'Literature');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (14, 'de', 'Literatur');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (14, 'es', 'Literatura');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (15, 'fr', 'Histoire');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (15, 'en', 'History');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (15, 'de', 'Geschichte');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (15, 'es', 'Historia');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (16, 'fr', 'Géographie');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (16, 'en', 'Geography');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (16, 'de', 'Geographie');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (16, 'es', 'Geografía');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (17, 'fr', 'Autre');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (17, 'en', 'Other');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (17, 'de', 'Sonstiges');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (17, 'es', 'Otro');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (100, 'fr', 'Audio - non spécifié');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (100, 'en', 'Audio - Not specified');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (100, 'de', 'Audio - Nicht angegeben');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (100, 'es', 'Audio - No especificado');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (101, 'fr', 'Jazz');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (101, 'en', 'Jazz');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (101, 'de', 'Jazz');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (101, 'es', 'Jazz');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (102, 'fr', 'Blues');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (102, 'en', 'Blues');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (102, 'de', 'Blues');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (102, 'es', 'Blues');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (103, 'fr', 'Rock');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (103, 'en', 'Rock');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (103, 'de', 'Rock');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (103, 'es', 'Rock');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (104, 'fr', 'Musique du monde');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (104, 'en', 'World music');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (104, 'de', 'Weltmusik');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (104, 'es', 'Música del mundo');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (105, 'fr', 'Classique');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (105, 'en', 'Classical');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (105, 'de', 'Klassik');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (105, 'es', 'Clásica');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (200, 'fr', 'Vidéo - non spécifié');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (200, 'en', 'Video - Not specified');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (200, 'de', 'Video - Nicht angegeben');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (200, 'es', 'Video - No especificado');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (201, 'fr', 'Vidéo - Fiction');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (201, 'en', 'Video - Fiction');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (201, 'de', 'Video - Fiktion');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (201, 'es', 'Video - Ficción');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (202, 'fr', 'Vidéo - Histoire');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (202, 'en', 'Video - History');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (202, 'de', 'Video - Geschichte');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (202, 'es', 'Video - Historia');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (203, 'fr', 'Vidéo - Art');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (203, 'en', 'Video - Art');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (203, 'de', 'Video - Kunst');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (203, 'es', 'Video - Arte');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (204, 'fr', 'Documentaire');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (204, 'en', 'Documentary');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (204, 'de', 'Dokumentation');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (204, 'es', 'Documental');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (205, 'fr', 'Vidéo - Musical');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (205, 'en', 'Video - Musical');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (205, 'de', 'Video - Musical');
        INSERT INTO genre_labels (genre_code, language, label) VALUES (205, 'es', 'Vídeo - Musical');
    END IF;
END
$seed_genre_labels$;

DO $seed_genre_synonyms$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM genre_synonyms LIMIT 1) THEN
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (1, 2, 'rom', 'rom', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (2, 2, 'roman', 'roman', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (3, 2, 'romans', 'romans', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (4, 2, 'Fiction', 'fiction', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (5, 2, 'Fiktion', 'fiktion', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (6, 2, 'Ficción', 'ficcion', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (7, 2, 'novel', 'novel', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (8, 2, 'novels', 'novels', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (9, 2, 'novela', 'novela', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (10, 2, 'novelas', 'novelas', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (11, 2, 'romance', 'romance', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (12, 2, 'romances', 'romances', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (13, 3, 'bd', 'bd', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (14, 3, 'bdc', 'bdc', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (15, 3, 'bande dessinée', 'bande dessinee', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (16, 3, 'bandes dessinées', 'bandes dessinees', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (17, 3, 'comic', 'comic', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (18, 3, 'comics', 'comics', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (19, 3, 'tebeo', 'tebeo', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (20, 3, 'tebeos', 'tebeos', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (21, 3, 'historieta', 'historieta', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (22, 3, 'graphic novel', 'graphic novel', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (23, 3, 'graphic novels', 'graphic novels', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (24, 3, 'manga', 'manga', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (25, 3, 'mangas', 'mangas', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (26, 4, 'the', 'the', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (27, 4, 'Théâtre', 'theatre', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (28, 4, 'Theater', 'theater', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (29, 4, 'Teatro', 'teatro', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (30, 4, 'drama', 'drama', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (31, 4, 'dramas', 'dramas', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (32, 5, 'poe', 'poe', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (33, 5, 'Poésie', 'poesie', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (34, 5, 'Poetry', 'poetry', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (35, 5, 'Poesía', 'poesia', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (36, 5, 'poem', 'poem', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (37, 5, 'poems', 'poems', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (38, 5, 'poèmes', 'poemes', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (39, 5, 'Gedicht', 'gedicht', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (40, 5, 'Gedichte', 'gedichte', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (41, 204, 'doc', 'doc', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (42, 204, 'Documentaire', 'documentaire', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (43, 204, 'Documentary', 'documentary', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (44, 204, 'Dokumentation', 'dokumentation', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (45, 204, 'Documental', 'documental', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (46, 204, 'documentaries', 'documentaries', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (47, 101, 'jaz', 'jaz', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (48, 101, 'Jazz', 'jazz', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (49, 102, 'blu', 'blu', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (50, 102, 'Blues', 'blues', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (51, 105, 'cla', 'cla', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (52, 105, 'Classique', 'classique', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (53, 105, 'Classical', 'classical', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (54, 105, 'Klassik', 'klassik', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (55, 105, 'Clásica', 'clasica', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (56, 105, 'musique classique', 'musique classique', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (57, 6, 'phi', 'phi', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (58, 6, 'Philosophie', 'philosophie', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (59, 6, 'Philosophy', 'philosophy', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (60, 6, 'Filosofía', 'filosofia', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (61, 15, 'his', 'his', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (62, 15, 'Histoire', 'histoire', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (63, 15, 'History', 'history', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (64, 15, 'Geschichte', 'geschichte', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (65, 15, 'Historia', 'historia', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (66, 15, 'biographie', 'biographie', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (67, 15, 'biography', 'biography', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (68, 16, 'geo', 'geo', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (69, 16, 'Géographie', 'geographie', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (70, 16, 'Geography', 'geography', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (71, 16, 'Geografía', 'geografia', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (72, 16, 'voyage', 'voyage', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (73, 16, 'travel', 'travel', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (74, 1, 'gen', 'gen', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (75, 1, 'Généralités', 'generalites', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (76, 1, 'General', 'general', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (77, 1, 'Allgemeines', 'allgemeines', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (78, 1, 'Generalidades', 'generalidades', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (79, 1, 'encyclopédie', 'encyclopedie', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (80, 1, 'encyclopedia', 'encyclopedia', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (81, 7, 'rel', 'rel', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (82, 7, 'Religion', 'religion', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (83, 7, 'religions', 'religions', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (84, 7, 'religiones', 'religiones', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (85, 8, 'soc', 'soc', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (86, 8, 'Sciences sociales', 'sciences sociales', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (87, 8, 'Social sciences', 'social sciences', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (88, 8, 'Sozialwissenschaften', 'sozialwissenschaften', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (89, 8, 'Ciencias sociales', 'ciencias sociales', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (90, 8, 'sociologie', 'sociologie', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (91, 8, 'sociology', 'sociology', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (92, 9, 'lan', 'lan', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (93, 9, 'Langues', 'langues', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (94, 9, 'Languages', 'languages', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (95, 9, 'Sprachen', 'sprachen', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (96, 9, 'Idiomas', 'idiomas', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (97, 9, 'linguistique', 'linguistique', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (98, 9, 'linguistics', 'linguistics', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (99, 10, 'sci', 'sci', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (100, 10, 'Sciences', 'sciences', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (101, 10, 'Wissenschaften', 'wissenschaften', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (102, 10, 'Ciencias', 'ciencias', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (103, 10, 'science', 'science', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (104, 11, 'tec', 'tec', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (105, 11, 'Technique', 'technique', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (106, 11, 'Technical', 'technical', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (107, 11, 'Technik', 'technik', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (108, 11, 'Técnica', 'tecnica', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (109, 11, 'technologie', 'technologie', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (110, 11, 'technology', 'technology', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (111, 11, 'informatique', 'informatique', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (112, 11, 'computer science', 'computer science', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (113, 12, 'art', 'art', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (114, 12, 'Arts', 'arts', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (115, 12, 'Kunst', 'kunst', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (116, 12, 'Arte', 'arte', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (117, 12, 'beaux arts', 'beaux arts', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (118, 12, 'fine arts', 'fine arts', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (119, 13, 'spo', 'spo', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (120, 13, 'Sport', 'sport', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (121, 13, 'sports', 'sports', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (122, 13, 'Deporte', 'deporte', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (123, 13, 'deportes', 'deportes', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (124, 14, 'lit', 'lit', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (125, 14, 'Littérature', 'litterature', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (126, 14, 'Literature', 'literature', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (127, 14, 'Literatur', 'literatur', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (128, 14, 'Literatura', 'literatura', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (129, 17, 'aut', 'aut', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (130, 17, 'Autre', 'autre', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (131, 17, 'Other', 'other', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (132, 17, 'Sonstiges', 'sonstiges', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (133, 17, 'Otro', 'otro', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (134, 103, 'roc', 'roc', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (135, 103, 'Rock', 'rock', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (136, 104, 'wld', 'wld', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (137, 104, 'World music', 'world music', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (138, 104, 'Musique du monde', 'musique du monde', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (139, 104, 'Weltmusik', 'weltmusik', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (140, 104, 'Música del mundo', 'musica del mundo', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (141, 201, 'vfi', 'vfi', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (142, 201, 'Video - Fiction', 'video fiction', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (143, 201, 'feature film', 'feature film', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (144, 201, 'films', 'films', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (145, 201, 'película', 'pelicula', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (146, 202, 'vhi', 'vhi', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (147, 202, 'Video - History', 'video history', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (148, 202, 'Vidéo - Histoire', 'video histoire', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (149, 203, 'var', 'var', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (150, 203, 'Video - Art', 'video art', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (151, 205, 'mus', 'mus', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (152, 205, 'musical', 'musical', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (153, 205, 'musicals', 'musicals', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (154, 205, 'Video - Musical', 'video musical', 'exact');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (155, 2, 'roman', 'roman', 'contains');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (156, 2, 'fiction', 'fiction', 'contains');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (157, 2, 'fiktion', 'fiktion', 'contains');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (158, 2, 'ficcion', 'ficcion', 'contains');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (159, 2, 'novel', 'novel', 'contains');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (160, 2, 'novela', 'novela', 'contains');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (161, 2, 'romance', 'romance', 'contains');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (162, 3, 'bande dessinee', 'bande dessinee', 'contains');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (163, 3, 'bandes dessinees', 'bandes dessinees', 'contains');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (164, 3, 'graphic novel', 'graphic novel', 'contains');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (165, 3, 'manga', 'manga', 'contains');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (166, 3, 'historieta', 'historieta', 'contains');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (167, 3, 'tebeo', 'tebeo', 'contains');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (168, 4, 'theatre', 'theatre', 'contains');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (169, 4, 'theater', 'theater', 'contains');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (170, 4, 'teatro', 'teatro', 'contains');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (171, 4, 'drama', 'drama', 'contains');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (172, 5, 'poesie', 'poesie', 'contains');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (173, 5, 'poetry', 'poetry', 'contains');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (174, 5, 'poesia', 'poesia', 'contains');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (175, 5, 'poem', 'poem', 'contains');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (176, 5, 'gedicht', 'gedicht', 'contains');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (177, 204, 'documentaire', 'documentaire', 'contains');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (178, 204, 'documentary', 'documentary', 'contains');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (179, 204, 'dokumentation', 'dokumentation', 'contains');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (180, 204, 'documental', 'documental', 'contains');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (181, 101, 'jazz', 'jazz', 'contains');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (182, 102, 'blues', 'blues', 'contains');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (183, 105, 'classique', 'classique', 'contains');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (184, 105, 'classical', 'classical', 'contains');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (185, 105, 'klassik', 'klassik', 'contains');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (186, 105, 'clasica', 'clasica', 'contains');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (187, 6, 'philosophie', 'philosophie', 'contains');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (188, 6, 'philosophy', 'philosophy', 'contains');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (189, 6, 'filosofia', 'filosofia', 'contains');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (190, 15, 'histoire', 'histoire', 'contains');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (191, 15, 'history', 'history', 'contains');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (192, 15, 'geschichte', 'geschichte', 'contains');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (193, 15, 'historia', 'historia', 'contains');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (194, 15, 'biograph', 'biograph', 'contains');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (195, 16, 'geographie', 'geographie', 'contains');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (196, 16, 'geography', 'geography', 'contains');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (197, 16, 'geografia', 'geografia', 'contains');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (198, 16, 'voyage', 'voyage', 'contains');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (199, 16, 'travel', 'travel', 'contains');
        INSERT INTO genre_synonyms (id, genre_code, term, term_normalized, match_kind) VALUES (200, 16, 'reisen', 'reisen', 'contains');
    END IF;
END
$seed_genre_synonyms$;
