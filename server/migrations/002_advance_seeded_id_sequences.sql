-- 001 inserts seed rows with explicit ids and never calls setval.
-- Fresh databases therefore hand out id 1 again and the next insert
-- collides with the seeded primary key (public types, loan settings, sites, …).

DO $$
DECLARE
    rec record;
    max_id bigint;
BEGIN
    FOR rec IN
        SELECT
            n.nspname AS schema_name,
            seq.relname AS sequence_name,
            tbl.relname AS table_name,
            attr.attname AS column_name
        FROM pg_class seq
        JOIN pg_namespace n ON n.oid = seq.relnamespace
        JOIN pg_depend dep ON dep.objid = seq.oid AND dep.deptype = 'a'
        JOIN pg_class tbl ON tbl.oid = dep.refobjid
        JOIN pg_attribute attr
            ON attr.attrelid = tbl.oid AND attr.attnum = dep.refobjsubid AND NOT attr.attisdropped
        WHERE seq.relkind = 'S'
          AND n.nspname = 'public'
    LOOP
        EXECUTE format(
            'SELECT COALESCE(MAX(%I), 0) FROM %I.%I',
            rec.column_name,
            rec.schema_name,
            rec.table_name
        ) INTO max_id;

        IF max_id > 0 THEN
            PERFORM setval(
                format('%I.%I', rec.schema_name, rec.sequence_name)::regclass,
                max_id,
                true
            );
        END IF;
    END LOOP;
END $$;
