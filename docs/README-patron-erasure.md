# Patron erasure (anonymize for stats)

French public libraries must erase or irreversibly anonymize patron personal data
on request or after the retention period (RGPD), while **keeping non-identifying
circulation statistics**.

Desk copy: **Anonymize for stats** (not “delete everything”). The API is still
`DELETE /api/v1/users/{id}`; `force=true` returns active loans first, then erases.

`users.id` is **never rewritten**. Identity is cut by setting `user_id` to `NULL`
on history. Stats use archive dimensions, not the person.

## Retention knobs

| Knob | Where | Default | Meaning |
|------|--------|---------|---------|
| `privacy.auto_anonymize_years_after_expiry` | `config/sample.toml`, `[privacy]`, env `ELIDUNE_PRIVACY__AUTO_ANONYMIZE_YEARS_AFTER_EXPIRY` | `0` (job off) | Years after `users.expiry_at` before the 04:00 scheduler anonymizes remaining patrons. A typical municipal-library value is `3`. Patrons with active loans are skipped (no force). |

The job emits `system.privacy_auto_erasure` (counts only) and one `user.deleted`
row per patron (`trigger: "retention_job"`). Audit payloads never store erased PII.

## `users` column checklist

| Column | On erasure |
|--------|------------|
| `id` | **Kept** (do not rewrite) |
| `account_type` | **Kept** (non-identifying classification; also snapshotted on archives) |
| `public_type` | **Kept** (audience dim; also snapshotted) |
| `created_at` | **Kept** (operational) |
| `expiry_at` | **Kept** (retention job / operational) |
| `status` | `deleted` |
| `archived_at` | `NOW()` if unset |
| `update_at` | `NOW()` |
| `token_version` | Incremented (revokes JWTs) |
| `login`, `password`, `firstname`, `lastname`, `email` | `NULL` |
| `addr_street`, `addr_zip_code`, `addr_city`, `phone` | `NULL` (`addr_city` snapshotted to archives first) |
| `barcode`, `notes`, `birthdate`, `sex`, `language` | `NULL` (age band snapshotted from birthdate + loan date) |
| `fee`, `group_id` | `NULL` |
| `staff_type`, `hours_per_week`, `staff_start_date`, `staff_end_date` | `NULL` |
| `two_factor_method`, `totp_secret`, `recovery_codes`, `recovery_codes_used` | `NULL` |
| `two_factor_enabled`, `receive_reminders`, `must_change_password` | `false` |

Pending `email_outbox` rows addressed to the patron’s email are deleted.

## History identity cut

| Table | Action |
|-------|--------|
| `loans_archives` | Fill missing dims, then `user_id = NULL` |
| `fines` | `user_id = NULL` (FK `ON DELETE SET NULL`) |
| `holds` | Deleted; next pending hold on each item is notified |
| `loans` | None left after a normal delete; `force` returns them into archives first |

## Archive stats dimensions

`loans_archives` already stored `borrower_public_type`, `addr_city`, `account_type`.
Erasure / return also persist:

| Column | Meaning |
|--------|---------|
| `age_band` | `0-17`, `18-29`, `30-49`, `50-64`, `65+` at **loan date** (same buckets as stats `users.age_band`) |
| `loan_year` | Calendar year of `date` |

Active unique-borrower counts that joined on `user_id` cannot follow an erased
patron. That is intentional: aggregates must use these dimensions.

## Audit

Successful desk erasure: `user.deleted` with
`{ id, force, anonymized, loansForceReturned, holdsCancelled, archivesUnlinked, finesUnlinked }`.
No name, contact, barcode, or secrets.
