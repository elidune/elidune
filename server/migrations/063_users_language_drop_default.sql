-- users.language defaulted to 'french', so a create that omitted language
-- stored an explicit French preference. Notices then skipped the library
-- language. NULL means no preference. Existing rows are not rewritten.

ALTER TABLE users ALTER COLUMN language DROP DEFAULT;
