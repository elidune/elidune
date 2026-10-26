-- Serials reference data and the serials right (MVP 1).
-- serial_subscriptions and serial_issues arrive with the phase that writes them.

ALTER TABLE account_types
    ADD COLUMN serials_rights character varying(1) DEFAULT 'n'::character varying NOT NULL;

COMMENT ON COLUMN account_types.serials_rights IS
    'n/r/w: none, read, or write access to serials (frequencies, numbering patterns, serial subscriptions). Distinct from items_rights and acquisitions_rights.';

UPDATE account_types
SET serials_rights = 'w'
WHERE code IN ('librarian', 'admin');

CREATE TABLE serial_frequencies (
    id bigint NOT NULL,
    description character varying(100) NOT NULL,
    unit character varying(10),
    issues_per_unit integer DEFAULT 1 NOT NULL,
    units_per_issue integer DEFAULT 1 NOT NULL,
    display_order integer DEFAULT 0 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT serial_frequencies_pkey PRIMARY KEY (id),
    CONSTRAINT serial_frequencies_unit_chk CHECK (
        unit IS NULL OR unit IN ('day', 'week', 'month', 'year')
    ),
    CONSTRAINT serial_frequencies_issues_chk CHECK (
        issues_per_unit >= 1 AND issues_per_unit <= 366
    ),
    CONSTRAINT serial_frequencies_units_chk CHECK (
        units_per_issue >= 1 AND units_per_issue <= 1000
    )
);

COMMENT ON TABLE serial_frequencies IS
    'Publication frequency for a serial subscription. NULL unit is irregular: dates are entered by hand.';

COMMENT ON COLUMN serial_frequencies.issues_per_unit IS
    'How many issues fall in one cycle of units_per_issue units.';

COMMENT ON COLUMN serial_frequencies.units_per_issue IS
    'How many units one cycle lasts. 1/2 weeks is unit=week, issues_per_unit=1, units_per_issue=2.';

CREATE INDEX idx_serial_frequencies_display_order
    ON serial_frequencies (display_order, id);

CREATE TABLE serial_numbering_patterns (
    id bigint NOT NULL,
    label character varying(100) NOT NULL,
    description text,
    display_order integer DEFAULT 0 NOT NULL,
    formula character varying(500) NOT NULL,
    locale character varying(8) DEFAULT 'fr'::character varying NOT NULL,
    name_x character varying(50),
    add_x integer DEFAULT 0 NOT NULL,
    every_x integer DEFAULT 1 NOT NULL,
    when_more_than_x integer,
    set_back_to_x integer,
    format_x character varying(20) DEFAULT 'number'::character varying NOT NULL,
    zero_pad_x integer,
    name_y character varying(50),
    add_y integer DEFAULT 0 NOT NULL,
    every_y integer DEFAULT 1 NOT NULL,
    when_more_than_y integer,
    set_back_to_y integer,
    format_y character varying(20) DEFAULT 'number'::character varying NOT NULL,
    zero_pad_y integer,
    name_z character varying(50),
    add_z integer DEFAULT 0 NOT NULL,
    every_z integer DEFAULT 1 NOT NULL,
    when_more_than_z integer,
    set_back_to_z integer,
    format_z character varying(20) DEFAULT 'number'::character varying NOT NULL,
    zero_pad_z integer,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT serial_numbering_patterns_pkey PRIMARY KEY (id),
    CONSTRAINT serial_numbering_patterns_locale_chk CHECK (locale IN ('fr', 'en')),
    CONSTRAINT serial_numbering_patterns_every_x_chk CHECK (every_x >= 1 AND every_x <= 10000),
    CONSTRAINT serial_numbering_patterns_every_y_chk CHECK (every_y >= 1 AND every_y <= 10000),
    CONSTRAINT serial_numbering_patterns_every_z_chk CHECK (every_z >= 1 AND every_z <= 10000),
    CONSTRAINT serial_numbering_patterns_reset_x_chk CHECK (
        (when_more_than_x IS NULL AND set_back_to_x IS NULL)
        OR (when_more_than_x IS NOT NULL AND set_back_to_x IS NOT NULL)
    ),
    CONSTRAINT serial_numbering_patterns_reset_y_chk CHECK (
        (when_more_than_y IS NULL AND set_back_to_y IS NULL)
        OR (when_more_than_y IS NOT NULL AND set_back_to_y IS NOT NULL)
    ),
    CONSTRAINT serial_numbering_patterns_reset_z_chk CHECK (
        (when_more_than_z IS NULL AND set_back_to_z IS NULL)
        OR (when_more_than_z IS NOT NULL AND set_back_to_z IS NOT NULL)
    ),
    CONSTRAINT serial_numbering_patterns_format_x_chk CHECK (
        format_x IN ('number', 'day_name', 'month_name', 'season')
    ),
    CONSTRAINT serial_numbering_patterns_format_y_chk CHECK (
        format_y IN ('number', 'day_name', 'month_name', 'season')
    ),
    CONSTRAINT serial_numbering_patterns_format_z_chk CHECK (
        format_z IN ('number', 'day_name', 'month_name', 'season')
    ),
    CONSTRAINT serial_numbering_patterns_pad_x_chk CHECK (
        zero_pad_x IS NULL OR (zero_pad_x >= 1 AND zero_pad_x <= 12)
    ),
    CONSTRAINT serial_numbering_patterns_pad_y_chk CHECK (
        zero_pad_y IS NULL OR (zero_pad_y >= 1 AND zero_pad_y <= 12)
    ),
    CONSTRAINT serial_numbering_patterns_pad_z_chk CHECK (
        zero_pad_z IS NULL OR (zero_pad_z >= 1 AND zero_pad_z <= 12)
    )
);

COMMENT ON TABLE serial_numbering_patterns IS
    'Koha-style numbering formula. Counters X, Y, and Z plus date tokens. The prediction engine renders display labels from this row.';

COMMENT ON COLUMN serial_numbering_patterns.formula IS
    'Display formula. Tokens: {X} {Y} {Z} {Year} {Month} {Day} {MonthName} {DayName}.';

CREATE INDEX idx_serial_numbering_patterns_display_order
    ON serial_numbering_patterns (display_order, id);

-- Koha sample frequencies. Ids stay small so they do not collide with snowflakes.
INSERT INTO serial_frequencies (
    id, description, unit, issues_per_unit, units_per_issue, display_order
) VALUES
    (1, '2/day', 'day', 2, 1, 1),
    (2, '1/day', 'day', 1, 1, 2),
    (3, '3/week', 'week', 3, 1, 3),
    (4, '1/week', 'week', 1, 1, 4),
    (5, '1/2 weeks', 'week', 1, 2, 5),
    (6, '1/3 weeks', 'week', 1, 3, 6),
    (7, '1/month', 'month', 1, 1, 7),
    (8, '1/2 months', 'month', 1, 2, 8),
    (9, '1/3 months', 'month', 1, 3, 9),
    (10, '2/month', 'month', 2, 1, 10),
    (11, '1/6 months', 'month', 1, 6, 11),
    (12, '1/year', 'year', 1, 1, 12),
    (13, '1/2 years', 'year', 1, 2, 13),
    (14, 'Irregular', NULL, 1, 1, 14);

-- Common numbering patterns. Ids stay small, like the frequency seed, so a
-- serial subscription can be created before staff write a pattern.
INSERT INTO serial_numbering_patterns (
    id, label, description, display_order, formula, locale,
    name_x, add_x, every_x, when_more_than_x, set_back_to_x, format_x, zero_pad_x,
    name_y, add_y, every_y, when_more_than_y, set_back_to_y, format_y, zero_pad_y,
    name_z, add_z, every_z, when_more_than_z, set_back_to_z, format_z, zero_pad_z
) VALUES
    (1, 'Number', 'Issue number', 1, 'n° {X}', 'fr',
     NULL, 1, 1, NULL, NULL, 'number', NULL,
     NULL, 0, 1, NULL, NULL, 'number', NULL,
     NULL, 0, 1, NULL, NULL, 'number', NULL),
    (2, 'Volume and number', 'Volume advances every 12 issues; the issue number returns to 1', 2, 'vol. {X}, n° {Y}', 'fr',
     NULL, 1, 12, NULL, NULL, 'number', NULL,
     NULL, 1, 1, 12, 1, 'number', NULL,
     NULL, 0, 1, NULL, NULL, 'number', NULL),
    (3, 'Number and date', 'Issue number with the month name and year', 3, 'n° {X}, {MonthName} {Year}', 'fr',
     NULL, 1, 1, NULL, NULL, 'number', NULL,
     NULL, 0, 1, NULL, NULL, 'number', NULL,
     NULL, 0, 1, NULL, NULL, 'number', NULL);
