# Squash Migrations Implementation Plan

> **For agentic workers:** implement task-by-task. Steps use checkbox syntax.

**Goal:** Single `001_initial_schema.sql` matching post-072 schema; drop incremental migrations and legacy DB import tooling.

**Architecture:** Migrate a throwaway DB through existing files, dump schema, rewrite as clean 001, delete 002–072, ship `reset_sqlx_migrations.sql` for prod tracking, adapt tests/docs.

**Tech Stack:** PostgreSQL, sqlx-cli, pg_dump, Rust tests.

## Global Constraints

- All code/comments in English.
- Do not modify domain “legacy” product features (withdrawals, notes).
- Keep `database/init_database.sql` symlink to `001`.

---

### Task 1: Dump final schema from empty DB

- [x] Create throwaway DB `elidune_squash`
- [x] `sqlx migrate run` with current migrations
- [x] `pg_dump --schema-only` (+ data for seed tables if needed)
- [x] Save dump under `/tmp` for rewrite

### Task 2: Rewrite `001_initial_schema.sql`

- [x] Replace `migrations/001_initial_schema.sql` with clean CREATE/seed/index/comment schema
- [x] Delete `migrations/002_*.sql` … `072_*.sql`
- [x] Verify symlink `database/init_database.sql`

### Task 3: Reset script + checksum

- [x] Compute sqlx checksum for new 001
- [x] Write `database/reset_sqlx_migrations.sql`
- [x] Document in `server/README.md`

### Task 4: Remove legacy DB tooling

- [x] Delete `database/legacy_db.sql`, `database/migrate_data.py`
- [x] Update README / AGENTS.md / `users.rs` comment

### Task 5: Fix migration `include_str!` tests

- [x] Adapt or remove tests referencing deleted migration files
- [x] `cargo test` / targeted integration tests

### Task 6: Verify fresh migrate

- [x] New empty DB + `sqlx migrate run` → only version 1
