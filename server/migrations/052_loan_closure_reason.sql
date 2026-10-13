-- Closed loans record why they closed (#157).
--
-- closure_reason: 'returned' | 'lost'
--   returned — physical return, damaged return, or a claims-returned case
--              resolved as found.
--   lost     — the copy was declared lost, or a claims-returned case was
--              resolved as not found. Finding the copy later does not change
--              this. The loan stays closed for loss.
-- found_at: when a copy closed as lost is later accepted back (circulate or
--           damaged). Null while it stays lost, and null on returned loans.
--
-- Backfill. Existing rows default to 'returned'. An archive is set to 'lost'
-- only when existing data shows the loan was closed because the copy was
-- declared lost. loans_archives does not keep the original loan id (the insert
-- allocates a new id and the loans row is deleted), so the match is the copy,
-- the borrower, and time:
--
-- 1. Successful audit item.marked_lost with payload loanClosed = true.
--    entity_id is the item. payload userId is the borrower (decimal text).
--    The archive is the row for that item and user whose returned_at is the
--    latest at or before the audit created_at, within 2 minutes. The audit is
--    written after the archive transaction commits.
-- 2. Successful audit item.claims_returned_resolved with loanClosed = true and
--    resolveOutcome = 'notFound'. Same item / user / time match. That outcome
--    closes the loan and marks the copy lost, and it does not create a
--    replacement charge.
-- 3. A replacement fine (fines.charge_type = 'replacement') for the same item
--    and user, when fines.item_id is set. Mark-lost is the only writer of
--    replacement charges, and it closes the loan in the same transaction.
--    The archive is the row whose returned_at is nearest that fine's
--    created_at, within 2 minutes.
-- 4. If the copy is still items.circulation_status = 1 (lost) and rules 1–3
--    did not match, the latest archive for that item is 'lost'. Declaring a
--    copy lost always archives the open loan and leaves the copy lost until
--    staff resolve it. A normal return does not set circulation_status to lost,
--    and a lost copy cannot be checked out again, so that latest archive is
--    the loss closure. This covers a lost declaration whose audit write failed
--    and that was not billed.
--
-- When none of those signals exist, the archive stays 'returned'.
--
-- found_at backfill: successful audit item.lost_found_resolved whose
-- disposition is 'circulate' or 'damaged' (not 'leave', which keeps the copy
-- lost). found_at is the earliest such audit's created_at on the latest
-- 'lost' archive for that item with returned_at <= created_at.

ALTER TABLE loans_archives
    ADD COLUMN IF NOT EXISTS closure_reason TEXT NOT NULL DEFAULT 'returned';

ALTER TABLE loans_archives
    DROP CONSTRAINT IF EXISTS loans_archives_closure_reason_check;

ALTER TABLE loans_archives
    ADD CONSTRAINT loans_archives_closure_reason_check
    CHECK (closure_reason IN ('returned', 'lost'));

ALTER TABLE loans_archives
    ADD COLUMN IF NOT EXISTS found_at TIMESTAMPTZ;

COMMENT ON COLUMN loans_archives.closure_reason IS
    'Why the loan was closed: returned (physical return, damaged return, or claims-returned found) or lost (declared lost, or claims-returned not found). Finding the copy later does not change this.';

COMMENT ON COLUMN loans_archives.found_at IS
    'When a copy closed as lost was later accepted back (circulate or damaged). Null while it stays lost, and null for returned loans.';

-- 1. Declared lost (audit).
UPDATE loans_archives AS la
SET closure_reason = 'lost'
FROM audit_log AS a
WHERE a.event_type = 'item.marked_lost'
  AND a.outcome = 'success'
  AND a.entity_type = 'item'
  AND a.payload->>'loanClosed' = 'true'
  AND a.payload->>'userId' ~ '^[0-9]+$'
  AND la.item_id = a.entity_id
  AND la.user_id = (a.payload->>'userId')::bigint
  AND la.returned_at IS NOT NULL
  AND la.returned_at <= a.created_at
  AND la.returned_at >= a.created_at - INTERVAL '2 minutes'
  AND la.id = (
      SELECT la2.id
      FROM loans_archives AS la2
      WHERE la2.item_id = a.entity_id
        AND la2.user_id = (a.payload->>'userId')::bigint
        AND la2.returned_at IS NOT NULL
        AND la2.returned_at <= a.created_at
        AND la2.returned_at >= a.created_at - INTERVAL '2 minutes'
      ORDER BY la2.returned_at DESC, la2.id DESC
      LIMIT 1
  );

-- 2. Claims-returned resolved as not found (audit).
UPDATE loans_archives AS la
SET closure_reason = 'lost'
FROM audit_log AS a
WHERE a.event_type = 'item.claims_returned_resolved'
  AND a.outcome = 'success'
  AND a.entity_type = 'item'
  AND a.payload->>'loanClosed' = 'true'
  AND a.payload->>'resolveOutcome' = 'notFound'
  AND a.payload->>'userId' ~ '^[0-9]+$'
  AND la.item_id = a.entity_id
  AND la.user_id = (a.payload->>'userId')::bigint
  AND la.returned_at IS NOT NULL
  AND la.returned_at <= a.created_at
  AND la.returned_at >= a.created_at - INTERVAL '2 minutes'
  AND la.id = (
      SELECT la2.id
      FROM loans_archives AS la2
      WHERE la2.item_id = a.entity_id
        AND la2.user_id = (a.payload->>'userId')::bigint
        AND la2.returned_at IS NOT NULL
        AND la2.returned_at <= a.created_at
        AND la2.returned_at >= a.created_at - INTERVAL '2 minutes'
      ORDER BY la2.returned_at DESC, la2.id DESC
      LIMIT 1
  );

-- 3. Replacement charge written when the copy was declared lost.
UPDATE loans_archives AS la
SET closure_reason = 'lost'
FROM fines AS f
WHERE f.charge_type = 'replacement'
  AND f.item_id IS NOT NULL
  AND la.item_id = f.item_id
  AND la.user_id = f.user_id
  AND la.returned_at IS NOT NULL
  AND la.returned_at <= f.created_at + INTERVAL '2 minutes'
  AND la.returned_at >= f.created_at - INTERVAL '2 minutes'
  AND la.id = (
      SELECT la2.id
      FROM loans_archives AS la2
      WHERE la2.item_id = f.item_id
        AND la2.user_id = f.user_id
        AND la2.returned_at IS NOT NULL
        AND la2.returned_at <= f.created_at + INTERVAL '2 minutes'
        AND la2.returned_at >= f.created_at - INTERVAL '2 minutes'
      ORDER BY ABS(EXTRACT(EPOCH FROM (la2.returned_at - f.created_at))) ASC,
               la2.id DESC
      LIMIT 1
  );

-- 4. Copy still lost, and no audit or replacement charge matched.
UPDATE loans_archives AS la
SET closure_reason = 'lost'
FROM items AS i
WHERE i.id = la.item_id
  AND i.circulation_status = 1
  AND la.closure_reason = 'returned'
  AND la.id = (
      SELECT la2.id
      FROM loans_archives AS la2
      WHERE la2.item_id = i.id
      ORDER BY la2.returned_at DESC NULLS LAST, la2.id DESC
      LIMIT 1
  );

-- found_at: earliest circulate/damaged resolution after the loss closure.
UPDATE loans_archives AS la
SET found_at = found.found_at
FROM (
    SELECT DISTINCT ON (la2.id)
           la2.id AS archive_id,
           a.created_at AS found_at
    FROM loans_archives AS la2
    JOIN audit_log AS a
      ON a.event_type = 'item.lost_found_resolved'
     AND a.outcome = 'success'
     AND a.entity_type = 'item'
     AND a.entity_id = la2.item_id
     AND a.payload->>'disposition' IN ('circulate', 'damaged')
    WHERE la2.closure_reason = 'lost'
      AND la2.returned_at IS NOT NULL
      AND la2.returned_at <= a.created_at
      AND la2.id = (
          SELECT la3.id
          FROM loans_archives AS la3
          WHERE la3.item_id = a.entity_id
            AND la3.closure_reason = 'lost'
            AND la3.returned_at IS NOT NULL
            AND la3.returned_at <= a.created_at
          ORDER BY la3.returned_at DESC, la3.id DESC
          LIMIT 1
      )
    ORDER BY la2.id, a.created_at ASC
) AS found
WHERE la.id = found.archive_id
  AND la.found_at IS NULL;
