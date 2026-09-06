# Holds — bibliographic (notice) queue

The hold **queue** is the bibliographic record (notice). There is one reservation type. A concrete copy is **allocated later**, when a specimen becomes available. Inter-site movement (`#15`) is item transit, not a second hold type.

## One type

| Field | Role |
|---|---|
| `biblioId` | Queue unit. Always stored. |
| `itemId` | Allocation. `null` while waiting for any copy; set when staff pin a specimen or when fulfillment assigns one. |
| `pickupSiteId` | Lives on the hold itself (not a second reservation). Transit moves the allocated copy toward this site, then the hold becomes `ready`. |

`POST /api/v1/holds` with `biblioId` (OPAC / desk title hold) or `itemId` (staff pin a copy). Both create the same row type on the same biblio queue.

## Fulfillment

When a copy becomes available (loan return, ready-hold cancel, ready-hold expiry):

1. If this specimen already has a `ready` hold, or an active transit, do nothing.
2. Otherwise offer the copy to the next `pending` hold on **that biblio** (position / `created_at` FIFO) that can take it:
   - unassigned (`itemId` null) — any copy
   - pinned to this specimen — this copy only
3. Same-site (no `pickupSiteId`, or it matches the copy's current `sourceId`): allocate, `status = ready`, `notifiedAt` / `expiresAt`.
4. Cross-site: allocate the copy, keep the hold `pending`, create an `item_transits` row (`requested`) toward **that hold’s** `pickupSiteId`. Staff ship → `in_transit` (not checkoutable). Receive at pickup marks the **same** hold `ready` and starts expiry. No parallel queue.

Cancel of a hold with an in-transit copy frees the reservation and opens a reverse transit back to the origin.

A hold pinned to a **different** copy is left in place; it does not block later patrons who can take this copy, and it is not a second queue.

Concurrent returns of two copies use `FOR UPDATE SKIP LOCKED` so the same hold cannot be notified twice.

## Uniqueness and the `#16` cap

- One active (`pending`/`ready`) hold per patron per biblio.
- Every active hold counts as **one** slot toward `maxActiveHolds`.

## Listing

| Endpoint | Contents |
|---|---|
| `GET /biblios/:id/holds` | The notice-level queue (FIFO) |
| `GET /items/:id/holds` | Holds already allocated or pinned to that copy |
| `GET /holds` | All holds (staff) or the caller’s holds (`own`) |

`HoldDetails.biblio.items` is empty until a copy is allocated; afterwards it contains exactly that specimen.
