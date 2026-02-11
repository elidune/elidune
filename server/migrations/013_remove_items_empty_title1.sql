-- =============================================================================
-- Migration 013: Remove items with empty title1 and related data
-- =============================================================================
-- This migration removes all items where title1 is NULL or empty,
-- along with their related specimens and loans.
-- =============================================================================

-- =============================================
-- STEP 1: Delete loans referencing specimens of items with empty title1
-- =============================================

DELETE FROM loans
WHERE specimen_id IN (
    SELECT s.id
    FROM specimens s
    INNER JOIN items i ON s.id_item = i.id
    WHERE i.title1 IS NULL OR TRIM(i.title1) = ''
);

-- =============================================
-- STEP 2: Delete archived loans referencing specimens of items with empty title1
-- =============================================

DELETE FROM loans_archives
WHERE specimen_id IN (
    SELECT s.id
    FROM specimens s
    INNER JOIN items i ON s.id_item = i.id
    WHERE i.title1 IS NULL OR TRIM(i.title1) = ''
);

-- =============================================
-- STEP 3: Delete specimens of items with empty title1
-- =============================================

DELETE FROM specimens
WHERE id_item IN (
    SELECT id
    FROM items
    WHERE title1 IS NULL OR TRIM(title1) = ''
);

-- =============================================
-- STEP 4: Delete items with empty title1
-- =============================================

DELETE FROM items
WHERE title1 IS NULL OR TRIM(title1) = '';

-- =============================================================================
-- Migration complete
-- =============================================================================
