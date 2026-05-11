-- Rename circulation/holds rights column; values remain n / r / w; add semantic 'o' (own) via API validation.
ALTER TABLE account_types
    RENAME COLUMN borrows_rights TO holds_rights;

COMMENT ON COLUMN account_types.holds_rights IS
    'n (none), o (own holds: self-service only), r (read queues/lists), w (full circulation + holds management)';

-- Patron-like types: restrict to self-service holds (was ''r'', which allowed queue inspection via API).
UPDATE account_types SET holds_rights = 'o'
WHERE code IN ('reader', 'group') AND holds_rights = 'r';
