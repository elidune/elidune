-- Tracks periodical catalog migration from one-notice-per-issue to serial subscriptions.

CREATE TABLE periodic_migration_sources (
    source_biblio_id bigint NOT NULL,
    canonical_biblio_id bigint NOT NULL,
    serial_subscription_id bigint NOT NULL,
    serial_issue_id bigint,
    migrated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT periodic_migration_sources_pkey PRIMARY KEY (source_biblio_id),
    CONSTRAINT periodic_migration_sources_source_fk FOREIGN KEY (source_biblio_id)
        REFERENCES biblios (id),
    CONSTRAINT periodic_migration_sources_canonical_fk FOREIGN KEY (canonical_biblio_id)
        REFERENCES biblios (id),
    CONSTRAINT periodic_migration_sources_subscription_fk FOREIGN KEY (serial_subscription_id)
        REFERENCES serial_subscriptions (id),
    CONSTRAINT periodic_migration_sources_issue_fk FOREIGN KEY (serial_issue_id)
        REFERENCES serial_issues (id) ON DELETE SET NULL
);

COMMENT ON TABLE periodic_migration_sources IS
    'One row per legacy periodical biblio merged into a serial subscription. Idempotency anchor for migrate-periodics apply.';

CREATE INDEX idx_periodic_migration_sources_canonical
    ON periodic_migration_sources (canonical_biblio_id);

CREATE INDEX idx_periodic_migration_sources_subscription
    ON periodic_migration_sources (serial_subscription_id);
