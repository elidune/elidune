# API Access Rights Matrix

This document lists API endpoints and their required authentication/authorization level.

## Auth levels

- `Public`: no token required
- `JWT (full)`: valid Bearer token, scope must not be `change_password_only` (`AuthenticatedUser`)
- `JWT (password-change scope)`: Bearer token with `scope == "change_password_only"` (`PasswordChangeUser`)
- `Staff`: authenticated user with `account_type` in `{librarian, admin}` (`StaffUser`)
- `Admin (extractor)`: authenticated user with `account_type == admin` (`AdminUser`)
- `JWT + require_*()`: authenticated user plus granular rights check from JWT claims

JWT rights fields in `UserRights` (JSON camelCase, e.g. `holdsRights`): `items_rights`, `users_rights`, `loans_rights`, `holds_rights`, `settings_rights`, `events_rights`.

For `items_rights`, `users_rights`, `loans_rights`, `settings_rights`, and `events_rights`, the level is **`none` \| `read` \| `write`** (from DB letters `n` / `r` / `w`). Checks use ordering: none < read < write.

**`holds_rights`** (DB column `account_types.holds_rights`) uses **`n` \| `o` \| `r` \| `w`**: **none**, **own** (self-service holds only), **read** (staff: queues and global hold lists), **write** (full circulation + holds management). Ordering for checks: none < own < read < write.  
Serialized JWT claims use `holdsRights`; **`borrowsRights`** is still accepted as a **deserialize alias** for backward compatibility.

Helpers on `UserClaims`:

| Method | Condition |
|---|---|
| `require_read_items()` | `items_rights >= read` |
| `require_write_items()` | `items_rights >= write` |
| `require_read_users()` | `users_rights >= read` |
| `require_write_users()` | `users_rights >= write` |
| `require_read_loans()` | `loans_rights >= read` |
| `require_write_holds()` | `holds_rights >= write` (circulation: checkout, return, renew, loan batches) |
| `require_read_holds_staff()` | `holds_rights >= read` (not satisfied by **`own`** alone) |
| `require_list_holds()` | `holds_rights >= read` **or** `holds_rights == own` |
| `require_create_hold()` | `holds_rights >= write` **or** `holds_rights == own` |
| `require_cancel_hold()` | `holds_rights >= write` **or** `holds_rights == own` |
| `require_read_settings()` | `settings_rights >= read` |
| `require_write_settings()` | `settings_rights >= write` |
| `require_read_events()` | `events_rights >= read` |
| `require_write_events()` | `events_rights >= write` |
| `require_admin()` | `account_type == admin` |
| `require_self_or_staff(id)` | caller is `id`, or `account_type` is librarian/admin |
| `require_self_or_admin(id)` | caller is `id`, or `account_type` is admin |

---

## Root and infrastructure

| Endpoint | Required auth |
|---|---|
| `GET /version` | Public |
| `GET /health` | Public |
| `GET /ready` | Public |
| `GET /swagger-ui/...` | Public |
| `GET /api-docs/openapi.json` | Public |

## Auth

All auth routes are rate-limited via GovernorLayer.

| Endpoint | Required auth |
|---|---|
| `POST /auth/login` | Public |
| `POST /auth/verify-2fa` | Public |
| `POST /auth/verify-recovery` | Public |
| `POST /auth/request-password-reset` | Public |
| `POST /auth/reset-password` | Public |
| `GET /auth/me` | JWT (full) |
| `PUT /auth/profile` | JWT (full) |
| `POST /auth/setup-2fa` | JWT (full) |
| `POST /auth/disable-2fa` | JWT (full) |
| `POST /auth/change-password` | JWT (password-change scope) |

## OPAC and public catalog

| Endpoint | Required auth |
|---|---|
| `GET /opac/biblios` | Public |
| `GET /opac/biblios/:id` | Public |
| `GET /opac/biblios/:id/availability` | Public |
| `GET /covers/isbn/:isbn` | Public |
| `GET /library-info` | Public |
| `PUT /library-info` | JWT + `require_write_settings()` |

## Biblios

| Endpoint | Required auth |
|---|---|
| `GET /biblios` | JWT + `require_read_items()` |
| `GET /biblios/:id` | JWT + `require_read_items()` |
| `POST /biblios` | JWT + `require_write_items()` |
| `PUT /biblios/:id` | JWT + `require_write_items()` |
| `DELETE /biblios/:id` | JWT + `require_write_items()` |
| `GET /biblios/:id/items` | JWT + `require_read_items()` |
| `GET /items/:id` | JWT + `require_read_items()` (biblio for that copy; `items` array length 1) |
| `POST /biblios/:id/items` | JWT + `require_write_items()` |
| `PUT /items/:id` | JWT + `require_write_items()` |
| `DELETE /items/:id` | JWT + `require_write_items()` |
| `GET /biblios/export.csv` | JWT + `require_read_items()` |
| `POST /biblios/load-marc` | JWT + `require_read_items()` |
| `POST /biblios/import-marc-batch` | JWT + `require_write_items()` |
| `GET /biblios/list-marc-batches` | JWT + `require_read_items()` |
| `GET /biblios/marc-batch/:batch_id` | JWT + `require_read_items()` |

## Series and collections

| Endpoint | Required auth |
|---|---|
| `GET /series` | JWT + `require_read_items()` |
| `GET /series/:id` | JWT + `require_read_items()` |
| `GET /series/:id/biblios` | JWT + `require_read_items()` |
| `POST /series` | JWT + `require_write_items()` |
| `PUT /series/:id` | JWT + `require_write_items()` |
| `DELETE /series/:id` | JWT + `require_write_items()` |
| `GET /collections` | JWT + `require_read_items()` |
| `GET /collections/:id` | JWT + `require_read_items()` |
| `GET /collections/:id/biblios` | JWT + `require_read_items()` |
| `POST /collections` | JWT + `require_write_items()` |
| `PUT /collections/:id` | JWT + `require_write_items()` |
| `DELETE /collections/:id` | JWT + `require_write_items()` |

## Sources

| Endpoint | Required auth |
|---|---|
| `GET /sources` | JWT + `require_read_items()` |
| `GET /sources/:id` | JWT + `require_read_items()` |
| `POST /sources` | JWT + `require_write_items()` |
| `PUT /sources/:id` | JWT + `require_write_items()` |
| `POST /sources/:id/archive` | JWT + `require_write_items()` |
| `POST /sources/merge` | JWT + `require_write_items()` |

## Users

| Endpoint | Required auth |
|---|---|
| `GET /users` | JWT + `require_read_users()` |
| `POST /users` | JWT + `require_write_users()` |
| `GET /users/:id` | JWT + `require_read_users()` |
| `PUT /users/:id` | JWT + `require_write_users()` |
| `DELETE /users/:id` | JWT + `require_write_users()` |
| `PUT /users/:id/account-type` | JWT + `require_admin()` |
| `PUT /users/:id/force-password-change` | JWT + `require_admin()` |
| `GET /users/:id/loans` | JWT + `require_read_users()` |
| `GET /users/:id/holds` | JWT + `require_read_holds_staff()` + `require_read_users()` |
| `GET /users/:id/fines` | JWT + `require_read_users()` |

## Loans and circulation

Loan checkout, return, renew, and batch loan operations require **`holds_rights >= write`** (same column as holds).

| Endpoint | Required auth |
|---|---|
| `POST /loans` | JWT + `require_write_holds()` |
| `POST /loans/:id/return` | JWT + `require_write_holds()` |
| `POST /loans/:id/renew` | JWT + `require_write_holds()` |
| `POST /loans/items/:item_id/return` | JWT + `require_write_holds()` |
| `POST /loans/items/:item_id/renew` | JWT + `require_write_holds()` |
| `GET /loans/overdue` | JWT + `require_read_loans()` |
| `POST /loans/send-overdue-reminders` | JWT + `require_admin()` |
| `POST /loans/batch-return` | JWT + `require_write_holds()` |
| `POST /loans/batch-create` | JWT + `require_write_holds()` |

## Holds

| Endpoint | Required auth | Notes |
|---|---|---|
| `GET /holds` | JWT + `require_list_holds()` | `read` / `write`: paginated **all** holds. **`own`**: same query, paginated **only the caller's** holds. |
| `POST /holds` | JWT + `require_create_hold()` | `write`: any `userId`. **`own`**: `userId` must be the caller. **`read`** alone: not allowed. |
| `GET /items/:id/holds` | JWT + `require_read_holds_staff()` | Hold queue for the item; not allowed for **`own`**. |
| `GET /users/:id/holds` | JWT + `require_read_holds_staff()` + `require_read_users()` | Not allowed for **`own`**. |
| `DELETE /holds/:id` | JWT + `require_cancel_hold()` | `write`: may cancel any user's hold. **`own`**: only own holds. **`read`** alone: not allowed. |

## Fines

| Endpoint | Required auth |
|---|---|
| `GET /users/:id/fines` | JWT + `require_read_users()` |
| `GET /fines/rules` | JWT + `require_read_settings()` |
| `PUT /fines/rules` | Staff |
| `POST /fines/:id/pay` | Staff |
| `POST /fines/:id/waive` | Staff |

## Inventory

| Endpoint | Required auth |
|---|---|
| All `/inventory` routes | Staff |

## History (GDPR)

| Endpoint | Required auth |
|---|---|
| `GET /users/:id/history` | JWT + `require_self_or_staff(id)` |
| `GET /users/:id/history/preference` | JWT + `require_self_or_staff(id)` |
| `PUT /users/:id/history/preference` | JWT + `require_self_or_admin(id)` |

## SSE and Z39.50

| Endpoint | Required auth |
|---|---|
| `GET /events/stream` | JWT (full) |
| `GET /z3950/search` | JWT + `require_read_items()` |
| `POST /z3950/import` | JWT + `require_write_items()` |

## Stats

| Endpoint | Required auth | Notes |
|---|---|---|
| `GET /stats` | JWT + `require_read_items()` | |
| `GET /stats/loans` | JWT + `require_read_loans()` | non-admin: scoped to own data; admin: global or `user_id` filter |
| `GET /stats/users` | JWT + `require_read_loans()` | |
| `GET /stats/catalog` | JWT + `require_read_items()` | |

## Settings domains

| Endpoint group | Read | Write |
|---|---|---|
| `/settings` | `require_read_settings()` | `require_write_settings()` |
| `/public-types` | `require_read_settings()` | `require_write_settings()` |
| `/equipment` | `require_read_settings()` | `require_write_settings()` |
| `/events` (cultural events) | `require_read_settings()` | `require_write_settings()` |
| `/schedules` | Public | `require_write_settings()` |
| `/visitor-counts` | `require_read_settings()` | `require_write_settings()` |

## Admin

| Endpoint | Required auth |
|---|---|
| `GET /admin/config` | JWT + `require_admin()` |
| `PUT /admin/config/:section` | JWT + `require_admin()` |
| `DELETE /admin/config/:section` | JWT + `require_admin()` |
| `POST /admin/config/email/test` | JWT + `require_admin()` |
| `POST /admin/reindex-search` | JWT + `require_admin()` |
| `GET /audit` | JWT + `require_admin()` |
| `GET /audit/export` | JWT + `require_admin()` |
| `POST /maintenance` | Admin (extractor — `AdminUser`) |
