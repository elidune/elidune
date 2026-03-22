-- Rename item_authors.role → function and normalise values to camelCase strings.

-- Drop the unique constraint that references the old column name
ALTER TABLE item_authors DROP CONSTRAINT IF EXISTS item_authors_item_id_author_id_role_key;

-- Rename and widen the column
ALTER TABLE item_authors RENAME COLUMN role TO function;
ALTER TABLE item_authors ALTER COLUMN function TYPE VARCHAR(50);

-- Map legacy integer codes (stored as strings from AuthorFunction enum),
-- MARC relator codes, and camelCase strings to the new canonical camelCase values.
-- Anything not recognised is set to NULL.
UPDATE item_authors SET function = CASE function
    -- Integer codes from the old AuthorFunction enum
    WHEN '70'  THEN 'author'
    WHEN '440' THEN 'illustrator'
    WHEN '730' THEN 'translator'
    WHEN '695' THEN 'scientificAdvisor'
    WHEN '340' THEN 'scientificAdvisor'
    WHEN '80'  THEN 'prefaceWriter'
    WHEN '600' THEN 'photographer'
    WHEN '651' THEN 'publishingDirector'
    WHEN '650' THEN 'publishingDirector'
    WHEN '230' THEN 'composer'
    -- MARC relator codes
    WHEN 'aut' THEN 'author'
    WHEN 'ill' THEN 'illustrator'
    WHEN 'trl' THEN 'translator'
    WHEN 'edt' THEN 'scientificAdvisor'
    WHEN 'aui' THEN 'prefaceWriter'
    WHEN 'pht' THEN 'photographer'
    WHEN 'pbd' THEN 'publishingDirector'
    WHEN 'cmp' THEN 'composer'
    -- Already-canonical camelCase values (idempotent re-run)
    WHEN 'author'             THEN 'author'
    WHEN 'illustrator'        THEN 'illustrator'
    WHEN 'translator'         THEN 'translator'
    WHEN 'scientificAdvisor'  THEN 'scientificAdvisor'
    WHEN 'prefaceWriter'      THEN 'prefaceWriter'
    WHEN 'photographer'       THEN 'photographer'
    WHEN 'publishingDirector' THEN 'publishingDirector'
    WHEN 'composer'           THEN 'composer'
    ELSE NULL
END;

-- Restore unique constraint with new column name
ALTER TABLE item_authors ADD CONSTRAINT item_authors_item_id_author_id_function_key
    UNIQUE (item_id, author_id, function);
