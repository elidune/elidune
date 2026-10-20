-- Rename catalog "sources" (physical sites) to sites across schema and indexes.

ALTER TABLE sources RENAME TO sites;

ALTER INDEX IF EXISTS sources_pkey RENAME TO sites_pkey;
ALTER INDEX IF EXISTS sources_default_unique RENAME TO sites_default_unique;

ALTER TABLE items RENAME COLUMN source_id TO site_id;
ALTER INDEX IF EXISTS idx_items_source_active RENAME TO idx_items_site_active;

COMMENT ON COLUMN items.site_id IS 'Physical site where the copy is held (sites.id).';

ALTER TABLE biblios RENAME COLUMN source_id TO site_id;
COMMENT ON COLUMN biblios.site_id IS 'Legacy column — not used by the application. Site is stored on items.site_id per copy.';

ALTER TABLE inventory_sessions RENAME COLUMN scope_source_id TO scope_site_id;
COMMENT ON COLUMN inventory_sessions.scope_site_id IS
    'When set, expected counts, missing list, scans, and consolidation only include active items with this site_id.';

DROP INDEX IF EXISTS idx_inventory_sessions_open_scope;
CREATE UNIQUE INDEX IF NOT EXISTS idx_inventory_sessions_open_scope
    ON inventory_sessions (
        COALESCE(scope_site_id, -1::bigint),
        COALESCE(scope_place, -1::smallint)
    )
    WHERE status = 'open';

ALTER TABLE item_transits RENAME COLUMN from_source_id TO from_site_id;
ALTER TABLE item_transits RENAME COLUMN to_source_id TO to_site_id;

ALTER TABLE schedule_closures RENAME COLUMN source_id TO site_id;
COMMENT ON COLUMN schedule_closures.site_id IS
    'NULL = network-wide closure (every site). Set = this site only (sites.id).';
