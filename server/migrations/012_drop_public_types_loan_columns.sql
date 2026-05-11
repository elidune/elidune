-- Borrowing rules for an audience live in public_type_loan_settings only.
-- Merge legacy public_types.max_loans / loan_duration_days into the default row
-- (public_type wins when set so custom values on public_types are not lost).

UPDATE public_type_loan_settings ptls
SET
    duration = COALESCE(pt.loan_duration_days, ptls.duration),
    nb_max = COALESCE(pt.max_loans, ptls.nb_max)
FROM public_types pt
WHERE ptls.public_type_id = pt.id
  AND ptls.media_type IS NULL;

INSERT INTO public_type_loan_settings (public_type_id, media_type, duration, nb_max, nb_renews, renew_at)
SELECT pt.id, NULL, pt.loan_duration_days, pt.max_loans, 2, 'now'
FROM public_types pt
WHERE NOT EXISTS (
    SELECT 1 FROM public_type_loan_settings x
    WHERE x.public_type_id = pt.id AND x.media_type IS NULL
);

ALTER TABLE public_types
    DROP COLUMN max_loans,
    DROP COLUMN loan_duration_days;
