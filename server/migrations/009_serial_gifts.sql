-- One-off periodical gifts: a registry row pointing at a normal item, no subscription.

CREATE TABLE serial_gifts (
    id bigint NOT NULL,
    item_id bigint NOT NULL,
    biblio_id bigint NOT NULL,
    site_id bigint NOT NULL,
    volume_label text NOT NULL,
    cover_date date,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT serial_gifts_pkey PRIMARY KEY (id),
    CONSTRAINT serial_gifts_item_fk FOREIGN KEY (item_id) REFERENCES items (id),
    CONSTRAINT serial_gifts_biblio_fk FOREIGN KEY (biblio_id) REFERENCES biblios (id),
    CONSTRAINT serial_gifts_site_fk FOREIGN KEY (site_id) REFERENCES sites (id),
    CONSTRAINT serial_gifts_item_uid UNIQUE (item_id),
    CONSTRAINT serial_gifts_volume_label_chk CHECK (btrim(volume_label) <> '')
);

CREATE INDEX serial_gifts_created_at_idx ON serial_gifts (created_at DESC);
CREATE INDEX serial_gifts_biblio_id_idx ON serial_gifts (biblio_id);
CREATE INDEX serial_gifts_site_id_idx ON serial_gifts (site_id);

COMMENT ON TABLE serial_gifts IS
    'Punctual periodical gifts. No serial_subscription. The physical copy is items; cover_date is optional.';
