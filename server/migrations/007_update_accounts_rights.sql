UPDATE account_types SET loans_rights = 'o'
WHERE code IN ('reader', 'group');

UPDATE account_types SET users_rights = 'o'
WHERE code IN ('reader', 'group');

UPDATE account_types SET events_rights = 'r'
WHERE code IN ('reader', 'group', 'guest');

UPDATE account_types SET users_rights = 'n'
WHERE code IN ('guest');