-- At most one active (unreturned) loan per physical copy.
-- Returns currently DELETE from `loans` and archive; `returned_at IS NULL` matches
-- the existing active-loan predicate used by checkout and counts.

WITH dupes AS (
    SELECT id
    FROM (
        SELECT id,
               ROW_NUMBER() OVER (
                   PARTITION BY item_id
                   ORDER BY date ASC NULLS LAST, id ASC
               ) AS rn
        FROM loans
        WHERE returned_at IS NULL
          AND item_id IS NOT NULL
    ) ranked
    WHERE rn > 1
),
archived AS (
    INSERT INTO loans_archives (
        user_id, item_id, date, nb_renews, expiry_at,
        returned_at, notes, borrower_public_type, addr_city, account_type
    )
    SELECT
        l.user_id,
        l.item_id,
        l.date,
        l.nb_renews,
        l.expiry_at,
        NOW(),
        l.notes,
        u.public_type,
        u.addr_city,
        u.account_type
    FROM loans l
    JOIN dupes d ON d.id = l.id
    LEFT JOIN users u ON u.id = l.user_id
)
DELETE FROM loans l
USING dupes d
WHERE l.id = d.id;

CREATE UNIQUE INDEX IF NOT EXISTS idx_loans_one_active_per_item
    ON loans (item_id)
    WHERE returned_at IS NULL;

COMMENT ON INDEX idx_loans_one_active_per_item IS
    'At most one unreturned loan per item (physical copy).';
