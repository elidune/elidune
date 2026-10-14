-- Integer euro cents for circulation money.
--
-- SQLx applies this file in one transaction. Do not add BEGIN/COMMIT.
-- audit_log is not rewritten.
--
-- items.price: TEXT decimal euros -> BIGINT cents.
--   Blank and NULL stay NULL (unknown, not a reject).
--   Accept surrounding spaces, '.' or ',' as the decimal mark, and a trailing
--   € or EUR. '12,50', '12,50 €', '12.5', and '15' convert.
--   More than two non-zero fractional digits, negatives, and other text are
--   rejected with no rounding: the raw text is kept in money_migration_rejects
--   and the price becomes NULL. A rejected item price sets price_deferred so
--   the copy stays in circulation.
--
-- NUMERIC euro columns become BIGINT via round(x * 100). A value with more
--   than two decimal places is not rounded. NOT NULL columns abort the
--   migration. Nullable columns are logged and stored as NULL.
--
-- public_types.subscription_price is already integer cents. It is widened
--   INTEGER -> BIGINT only and is never multiplied by 100.
--
-- paid_amount + refunded_amount <= amount is NOT added. A collected charge
--   that is later refunded keeps paid_amount and sets refunded_amount to the
--   same collected sum, so the sum is larger than amount whenever something
--   was paid. The existing refunded_amount <= paid_amount check stays.
--   Rows that already violate the unpaid predicate are logged and left as-is.

CREATE TABLE IF NOT EXISTS money_migration_rejects (
    id          BIGSERIAL PRIMARY KEY,
    table_name  TEXT NOT NULL,
    row_id      BIGINT NOT NULL,
    column_name TEXT NOT NULL,
    raw_value   TEXT,
    migrated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

COMMENT ON TABLE money_migration_rejects IS
    'Legacy money values that could not be stored as integer cents. No foreign keys: the source row may later be deleted. raw_value is the pre-migration text.';

CREATE OR REPLACE FUNCTION elidune_migrate_item_price(raw text)
RETURNS TABLE(cents bigint, kind text)
LANGUAGE plpgsql
VOLATILE
AS $$
DECLARE
    s text;
    sep text;
    whole text;
    frac text;
BEGIN
    IF raw IS NULL OR btrim(raw) = '' THEN
        cents := NULL;
        kind := 'blank';
        RETURN NEXT;
        RETURN;
    END IF;

    s := btrim(raw);
    IF s ~* '[[:space:]]*(€|EUR)[[:space:]]*$' THEN
        s := btrim(regexp_replace(s, '[[:space:]]*(€|EUR)[[:space:]]*$', '', 'i'));
    END IF;

    IF s = '' OR left(s, 1) = '-' OR s !~ '^[+]?[0-9]+([.,][0-9]+)?$' THEN
        cents := NULL;
        kind := 'reject';
        RETURN NEXT;
        RETURN;
    END IF;

    IF left(s, 1) = '+' THEN
        s := substring(s FROM 2);
    END IF;

    IF position('.' IN s) > 0 AND position(',' IN s) > 0 THEN
        cents := NULL;
        kind := 'reject';
        RETURN NEXT;
        RETURN;
    END IF;

    IF position('.' IN s) > 0 THEN
        sep := '.';
    ELSIF position(',' IN s) > 0 THEN
        sep := ',';
    ELSE
        sep := NULL;
    END IF;

    IF sep IS NULL THEN
        whole := s;
        frac := '';
    ELSE
        whole := split_part(s, sep, 1);
        frac := split_part(s, sep, 2);
    END IF;

    WHILE length(frac) > 0 AND right(frac, 1) = '0' LOOP
        frac := left(frac, length(frac) - 1);
    END LOOP;

    IF length(frac) > 2 THEN
        cents := NULL;
        kind := 'reject';
        RETURN NEXT;
        RETURN;
    END IF;

    frac := rpad(COALESCE(frac, ''), 2, '0');

    BEGIN
        cents := (whole || frac)::bigint;
    EXCEPTION
        WHEN numeric_value_out_of_range OR invalid_text_representation THEN
            cents := NULL;
            kind := 'reject';
            RETURN NEXT;
            RETURN;
    END;

    kind := 'ok';
    RETURN NEXT;
END;
$$;

ALTER TABLE items ADD COLUMN IF NOT EXISTS price_deferred BOOLEAN NOT NULL DEFAULT FALSE;

ALTER TABLE items ADD COLUMN price_cents BIGINT;
ALTER TABLE items ADD COLUMN price_parse_kind TEXT;

UPDATE items AS i
SET price_cents = parsed.cents,
    price_parse_kind = parsed.kind
FROM (
    SELECT s.id, r.cents, r.kind
    FROM items AS s
    CROSS JOIN LATERAL elidune_migrate_item_price(s.price) AS r
) AS parsed
WHERE i.id = parsed.id;

INSERT INTO money_migration_rejects (table_name, row_id, column_name, raw_value)
SELECT 'items', id, 'price', price
FROM items
WHERE price_parse_kind = 'reject';

UPDATE items
SET price_deferred = TRUE
WHERE price_parse_kind = 'reject';

ALTER TABLE items DROP COLUMN price;
ALTER TABLE items RENAME COLUMN price_cents TO price;
ALTER TABLE items DROP COLUMN price_parse_kind;

ALTER TABLE items DROP CONSTRAINT IF EXISTS items_price_nonnegative;
ALTER TABLE items
    ADD CONSTRAINT items_price_nonnegative
    CHECK (price IS NULL OR price >= 0);

COMMENT ON COLUMN items.price IS
    'Purchase price in integer euro cents. NULL means unknown. 0 is a free copy, not unknown.';
COMMENT ON COLUMN items.price_deferred IS
    'True when a missing price is an explicit deferral, including a legacy price rejected by the cents migration, so the copy can stay in circulation.';

-- Abort rather than round or null a required euro amount that is not exact cents.
DO $preflight$
DECLARE
    sample text;
BEGIN
    SELECT detail INTO sample
    FROM (
        SELECT format('fines.amount id %s value %s', id, amount) AS detail
        FROM fines
        WHERE amount < 0 OR amount <> round(amount, 2) OR scale(amount) > 2
        UNION ALL
        SELECT format('fines.paid_amount id %s value %s', id, paid_amount)
        FROM fines
        WHERE paid_amount < 0 OR paid_amount <> round(paid_amount, 2) OR scale(paid_amount) > 2
        UNION ALL
        SELECT format('fines.refunded_amount id %s value %s', id, refunded_amount)
        FROM fines
        WHERE refunded_amount < 0
           OR refunded_amount <> round(refunded_amount, 2)
           OR scale(refunded_amount) > 2
        UNION ALL
        SELECT format('fine_rules.daily_rate id %s value %s', id, daily_rate)
        FROM fine_rules
        WHERE daily_rate < 0 OR daily_rate <> round(daily_rate, 2) OR scale(daily_rate) > 2
        UNION ALL
        SELECT format('circulation_settings.unpaid_fine_threshold id %s value %s', id, unpaid_fine_threshold)
        FROM circulation_settings
        WHERE unpaid_fine_threshold < 0
           OR unpaid_fine_threshold <> round(unpaid_fine_threshold, 2)
           OR scale(unpaid_fine_threshold) > 2
    ) AS bad
    LIMIT 1;

    IF sample IS NOT NULL THEN
        RAISE EXCEPTION
            'money migration aborted: % has more than 2 decimal places or is negative',
            sample;
    END IF;
END
$preflight$;

INSERT INTO money_migration_rejects (table_name, row_id, column_name, raw_value)
SELECT 'fine_rules', id::bigint, 'max_amount', max_amount::text
FROM fine_rules
WHERE max_amount IS NOT NULL
  AND (
      max_amount < 0
      OR max_amount <> round(max_amount, 2)
      OR scale(max_amount) > 2
  );

INSERT INTO money_migration_rejects (table_name, row_id, column_name, raw_value)
SELECT 'public_types', id, 'unpaid_fine_threshold', unpaid_fine_threshold::text
FROM public_types
WHERE unpaid_fine_threshold IS NOT NULL
  AND (
      unpaid_fine_threshold < 0
      OR unpaid_fine_threshold <> round(unpaid_fine_threshold, 2)
      OR scale(unpaid_fine_threshold) > 2
  );

ALTER TABLE fines
    ALTER COLUMN amount TYPE BIGINT USING round(amount * 100)::bigint,
    ALTER COLUMN paid_amount TYPE BIGINT USING round(paid_amount * 100)::bigint,
    ALTER COLUMN refunded_amount TYPE BIGINT USING round(refunded_amount * 100)::bigint;

ALTER TABLE fine_rules
    ALTER COLUMN daily_rate TYPE BIGINT USING round(daily_rate * 100)::bigint,
    ALTER COLUMN max_amount TYPE BIGINT USING (
        CASE
            WHEN max_amount IS NULL THEN NULL
            WHEN max_amount < 0
              OR max_amount <> round(max_amount, 2)
              OR scale(max_amount) > 2 THEN NULL
            ELSE round(max_amount * 100)::bigint
        END
    );

ALTER TABLE circulation_settings
    ALTER COLUMN unpaid_fine_threshold TYPE BIGINT
    USING round(unpaid_fine_threshold * 100)::bigint;

-- subscription_price is already cents. Widen only.
ALTER TABLE public_types
    ALTER COLUMN subscription_price TYPE BIGINT USING subscription_price::bigint,
    ALTER COLUMN unpaid_fine_threshold TYPE BIGINT USING (
        CASE
            WHEN unpaid_fine_threshold IS NULL THEN NULL
            WHEN unpaid_fine_threshold < 0
              OR unpaid_fine_threshold <> round(unpaid_fine_threshold, 2)
              OR scale(unpaid_fine_threshold) > 2 THEN NULL
            ELSE round(unpaid_fine_threshold * 100)::bigint
        END
    );

ALTER TABLE public_types DROP CONSTRAINT IF EXISTS public_types_subscription_price_nonnegative;
ALTER TABLE public_types
    ADD CONSTRAINT public_types_subscription_price_nonnegative
    CHECK (subscription_price IS NULL OR subscription_price >= 0);

COMMENT ON COLUMN public_types.subscription_price IS
    'Subscription price in integer euro cents (1500 = 15.00 EUR). Not a major-unit amount.';
COMMENT ON COLUMN public_types.unpaid_fine_threshold IS
    'Unpaid-fine block in integer euro cents. Used only when unpaid_threshold_mode is amount. 0 blocks any positive unpaid balance.';
COMMENT ON COLUMN fines.amount IS 'Charge total in integer euro cents.';
COMMENT ON COLUMN fines.paid_amount IS 'Amount collected in integer euro cents.';
COMMENT ON COLUMN fines.refunded_amount IS
    'Amount given back in integer euro cents. Never exceeds paid_amount. paid_amount is kept, so paid_amount + refunded_amount can exceed amount after a refund.';
COMMENT ON COLUMN fine_rules.daily_rate IS 'Overdue daily rate in integer euro cents.';
COMMENT ON COLUMN fine_rules.max_amount IS 'Overdue cap in integer euro cents. NULL means no cap.';
COMMENT ON COLUMN circulation_settings.unpaid_fine_threshold IS
    'Global unpaid-fine block in integer euro cents. Used only when unpaid_threshold_mode is amount.';

-- Existing >= 0 checks on fines, fine_rules, and both threshold columns are
-- revalidated by the type change above. The refund sum check is intentionally
-- omitted: see the header comment.
DO $refund_sum$
DECLARE
    offenders bigint;
BEGIN
    SELECT count(*) INTO offenders
    FROM fines
    WHERE paid_amount + refunded_amount > amount;

    IF offenders > 0 THEN
        INSERT INTO money_migration_rejects (table_name, row_id, column_name, raw_value)
        SELECT
            'fines',
            id,
            'paid_amount+refunded_amount',
            paid_amount::text || '+' || refunded_amount::text || '>' || amount::text
        FROM fines
        WHERE paid_amount + refunded_amount > amount;
    END IF;

    RAISE NOTICE
        'skipped fines paid_amount + refunded_amount <= amount (% offending row(s) logged, amounts unchanged). A refund keeps paid_amount and sets refunded_amount, so the sum exceeds amount after money is given back. refunded_amount <= paid_amount stays.',
        offenders;
END
$refund_sum$;

DO $notice$
DECLARE
    n bigint;
BEGIN
    SELECT count(*) INTO n FROM money_migration_rejects;
    RAISE NOTICE 'money migration rejects: %', n;
END
$notice$;

DROP FUNCTION elidune_migrate_item_price(text);
