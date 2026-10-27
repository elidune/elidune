-- Remember when the latest-issue rule itself made a copy non-borrowable.
-- Migrations 001–005 are unchanged.

ALTER TABLE serial_issues
    ADD COLUMN library_only_borrowable_before boolean;

COMMENT ON COLUMN serial_issues.library_only_borrowable_before IS
    'Set only while the latest-issue rule is holding this copy non-borrowable. The value is items.borrowable from just before the rule cleared it. Null means the rule is not holding this copy, so a later receipt must not force it borrowable.';
