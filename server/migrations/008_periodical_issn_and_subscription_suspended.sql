-- Periodical ISSN on the bibliographic record (MARC 022).
-- Serial subscription pause status: suspended (distinct from closed / expired).

ALTER TABLE biblios
    ADD COLUMN issn character varying(30);

COMMENT ON COLUMN biblios.issn IS
    'ISSN of this bibliographic resource (MARC 022). Distinct from series.issn / collections.issn (series statement).';

CREATE INDEX idx_biblios_issn
    ON biblios (issn)
    WHERE issn IS NOT NULL AND btrim(issn) <> '';

-- Prefer an ISSN already linked via series for existing periodical titles.
UPDATE biblios b
SET issn = sub.issn
FROM (
    SELECT DISTINCT ON (bs.biblio_id)
        bs.biblio_id,
        se.issn
    FROM biblio_series bs
    JOIN series se ON se.id = bs.series_id
    JOIN biblios b2 ON b2.id = bs.biblio_id
    WHERE b2.media_type = 'periodic'
      AND se.issn IS NOT NULL
      AND btrim(se.issn) <> ''
    ORDER BY bs.biblio_id, se.id
) sub
WHERE b.id = sub.biblio_id
  AND (b.issn IS NULL OR btrim(b.issn) = '');

-- Fallback: collection ISSN when no series ISSN was found.
UPDATE biblios b
SET issn = sub.issn
FROM (
    SELECT DISTINCT ON (bc.biblio_id)
        bc.biblio_id,
        c.issn
    FROM biblio_collections bc
    JOIN collections c ON c.id = bc.collection_id
    JOIN biblios b2 ON b2.id = bc.biblio_id
    WHERE b2.media_type = 'periodic'
      AND c.issn IS NOT NULL
      AND btrim(c.issn) <> ''
    ORDER BY bc.biblio_id, c.id
) sub
WHERE b.id = sub.biblio_id
  AND (b.issn IS NULL OR btrim(b.issn) = '');

ALTER TABLE serial_subscriptions
    DROP CONSTRAINT serial_subscriptions_status_chk;

ALTER TABLE serial_subscriptions
    ADD CONSTRAINT serial_subscriptions_status_chk CHECK (
        status IN ('active', 'expired', 'closed', 'suspended')
    );

COMMENT ON COLUMN serial_subscriptions.status IS
    'active: open. expired: paid period ended. closed: stopped permanently. suspended: paused; same cursor resumes on reactivation.';
