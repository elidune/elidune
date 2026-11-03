# Serial gifts Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Centralize one-off periodical gifts under Serials (`/serials?tab=gifts`) with list + create, no subscription.

**Architecture:** `serial_gifts` table; SerialsService creates the copy via `CatalogService::create_item` then inserts the gift row; UI tab mirrors Acquisitions tab URL pattern.

**Tech Stack:** Rust/Axum/SQLx, React/TanStack Query, existing Serials rights and Item::received_copy.

### Task 1: Migration + models
### Task 2: Repository + service + API routes
### Task 3: UI tab, gifts panel, i18n
### Task 4: Biblio deep-link + verify (tsc / cargo check)
