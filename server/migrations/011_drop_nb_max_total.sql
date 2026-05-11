-- Total concurrent loans use `nb_max` on the default row (media_type IS NULL), not a separate column.
ALTER TABLE loans_settings DROP COLUMN IF EXISTS nb_max_total;
ALTER TABLE public_type_loan_settings DROP COLUMN IF EXISTS nb_max_total;
