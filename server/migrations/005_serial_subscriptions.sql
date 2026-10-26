-- Serial subscriptions per site and the predicted issue rows they own (MVP 2).
-- Migration 004 is unchanged. Receipt, claims, and the scheduler are later phases.

CREATE TABLE serial_subscriptions (
    id bigint NOT NULL,
    biblio_id bigint NOT NULL,
    site_id bigint NOT NULL,
    vendor_id bigint,
    fund_id bigint,
    cost numeric(14,2),
    issue_price_cents integer,
    frequency_id bigint NOT NULL,
    numbering_pattern_id bigint NOT NULL,
    first_issue_date date NOT NULL,
    start_date date,
    end_date date,
    value_x integer NOT NULL,
    value_y integer NOT NULL,
    value_z integer NOT NULL,
    inner_x integer DEFAULT 0 NOT NULL,
    inner_y integer DEFAULT 0 NOT NULL,
    inner_z integer DEFAULT 0 NOT NULL,
    next_slot integer DEFAULT 1 NOT NULL,
    length_kind character varying(10) NOT NULL,
    length_count integer NOT NULL,
    irregularities integer[] DEFAULT '{}'::integer[] NOT NULL,
    recurring_skips jsonb DEFAULT '[]'::jsonb NOT NULL,
    combined_issues jsonb DEFAULT '[]'::jsonb NOT NULL,
    grace_days integer,
    status character varying(10) DEFAULT 'active'::character varying NOT NULL,
    create_items boolean DEFAULT true NOT NULL,
    latest_issue_library_use_only boolean DEFAULT true NOT NULL,
    holdings_statement text,
    call_number character varying(100),
    location character varying(200),
    public_note text,
    internal_note text,
    notes text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT serial_subscriptions_pkey PRIMARY KEY (id),
    CONSTRAINT serial_subscriptions_biblio_fk FOREIGN KEY (biblio_id) REFERENCES biblios (id),
    CONSTRAINT serial_subscriptions_site_fk FOREIGN KEY (site_id) REFERENCES sites (id),
    CONSTRAINT serial_subscriptions_vendor_fk FOREIGN KEY (vendor_id) REFERENCES vendors (id) ON DELETE SET NULL,
    CONSTRAINT serial_subscriptions_fund_fk FOREIGN KEY (fund_id) REFERENCES acquisition_funds (id) ON DELETE SET NULL,
    CONSTRAINT serial_subscriptions_frequency_fk FOREIGN KEY (frequency_id) REFERENCES serial_frequencies (id),
    CONSTRAINT serial_subscriptions_pattern_fk FOREIGN KEY (numbering_pattern_id) REFERENCES serial_numbering_patterns (id),
    CONSTRAINT serial_subscriptions_cost_chk CHECK (cost IS NULL OR cost >= 0),
    CONSTRAINT serial_subscriptions_issue_price_chk CHECK (
        issue_price_cents IS NULL OR issue_price_cents >= 0
    ),
    CONSTRAINT serial_subscriptions_inner_chk CHECK (
        inner_x >= 0 AND inner_y >= 0 AND inner_z >= 0
    ),
    CONSTRAINT serial_subscriptions_next_slot_chk CHECK (next_slot >= 1),
    CONSTRAINT serial_subscriptions_length_kind_chk CHECK (
        length_kind IN ('issues', 'weeks', 'months')
    ),
    CONSTRAINT serial_subscriptions_length_count_chk CHECK (
        length_count >= 1 AND length_count <= 10000
    ),
    CONSTRAINT serial_subscriptions_grace_chk CHECK (grace_days IS NULL OR grace_days >= 0),
    CONSTRAINT serial_subscriptions_status_chk CHECK (
        status IN ('active', 'expired', 'closed')
    ),
    CONSTRAINT serial_subscriptions_dates_chk CHECK (
        start_date IS NULL OR end_date IS NULL OR end_date >= start_date
    )
);

COMMENT ON TABLE serial_subscriptions IS
    'One periodical subscription at a site. cost is the annual price and is never copied onto an item. issue_price_cents is the optional per-issue price.';

COMMENT ON COLUMN serial_subscriptions.first_issue_date IS
    'Immutable prediction anchor. Horizons keep this date and continue from next_slot.';

COMMENT ON COLUMN serial_subscriptions.next_slot IS
    'Absolute 1-based slot the next horizon passes to the prediction engine as startSlot.';

COMMENT ON COLUMN serial_subscriptions.grace_days IS
    'Days after planned_date before a later job may mark an issue late. NULL uses the frequency default from serials::grace. 0 is an explicit override.';

COMMENT ON COLUMN serial_subscriptions.create_items IS
    'When true, receipt (MVP 3) creates an item. Default true; daily press is usually unchecked.';

COMMENT ON COLUMN serial_subscriptions.cost IS
    'Annual serial subscription cost. Receipt must not copy this to items.price.';

CREATE UNIQUE INDEX serial_subscriptions_one_active_uidx
    ON serial_subscriptions (biblio_id, site_id)
    WHERE status = 'active';

CREATE INDEX idx_serial_subscriptions_site_status
    ON serial_subscriptions (site_id, status);

CREATE INDEX idx_serial_subscriptions_vendor
    ON serial_subscriptions (vendor_id)
    WHERE vendor_id IS NOT NULL;

CREATE INDEX idx_serial_subscriptions_biblio
    ON serial_subscriptions (biblio_id);

CREATE TABLE serial_issues (
    id bigint NOT NULL,
    serial_subscription_id bigint NOT NULL,
    planned_date date NOT NULL,
    display_label text NOT NULL,
    sequence integer NOT NULL,
    slot integer NOT NULL,
    value_x integer NOT NULL,
    value_y integer NOT NULL,
    value_z integer NOT NULL,
    status character varying(20) DEFAULT 'expected'::character varying NOT NULL,
    supplement boolean DEFAULT false NOT NULL,
    item_id bigint,
    received_at timestamp with time zone,
    notes text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT serial_issues_pkey PRIMARY KEY (id),
    CONSTRAINT serial_issues_subscription_fk FOREIGN KEY (serial_subscription_id)
        REFERENCES serial_subscriptions (id),
    CONSTRAINT serial_issues_item_fk FOREIGN KEY (item_id) REFERENCES items (id) ON DELETE SET NULL,
    CONSTRAINT serial_issues_sequence_chk CHECK (sequence >= 1),
    CONSTRAINT serial_issues_slot_chk CHECK (slot >= 1),
    CONSTRAINT serial_issues_status_chk CHECK (
        status IN (
            'expected', 'arrived', 'late', 'missing', 'claimed',
            'not_available', 'deleted', 'bound'
        )
    ),
    CONSTRAINT serial_issues_subscription_slot_key UNIQUE (serial_subscription_id, slot),
    CONSTRAINT serial_issues_subscription_sequence_key UNIQUE (serial_subscription_id, sequence)
);

COMMENT ON TABLE serial_issues IS
    'Persisted prediction rows. display_label is what the desk, OPAC, holds, and claims show. Generating the next horizon appends rows; it does not rewrite existing ones.';

COMMENT ON COLUMN serial_issues.display_label IS
    'Human-readable numbering and date, stored at prediction time.';

COMMENT ON COLUMN serial_issues.slot IS
    'Absolute publication slot from the subscription anchor, including skipped irregularities.';

CREATE INDEX idx_serial_issues_subscription_date
    ON serial_issues (serial_subscription_id, planned_date);
