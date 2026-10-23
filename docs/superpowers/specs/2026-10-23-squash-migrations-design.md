# Squash migrations to 001 + drop legacy DB tooling

**Status:** approved  
**Date:** 2026-10-23

## Goal

All production Elidune databases are at migration 072. Collapse `002`–`072` into a single canonical `001_initial_schema.sql` for fresh installs, and remove legacy-system DB migration tooling.

## Decisions

- **Schema generation:** empty Postgres → apply current migrations → dump → rewrite as human-readable `001` (style of today’s file).
- **Prod `_sqlx_migrations`:** one-shot idempotent script `database/reset_sqlx_migrations.sql` (delete all rows, insert version 1 with new checksum). Run once per instance before deploying the squashed binary.
- **Legacy DB:** delete `database/legacy_db.sql`, `database/migrate_data.py`, and related docs/comments. Keep domain “legacy” API concepts (withdrawals, etc.).

## Out of scope

- Changing application schema beyond reflecting post-072 state.
- Reworking withdrawal/notes “legacy” product features.

## Verification

- Fresh DB: `sqlx migrate run` records only version 1.
- Existing DB: after reset script, server starts without migration checksum errors.
- `cargo test` / CI server job green; migration `include_str!` tests adapted or removed.
