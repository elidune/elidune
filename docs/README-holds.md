# Holds — bibliographic (notice) queue

The hold **queue** is the bibliographic record (notice). There is one reservation type. A concrete copy is **allocated later**, when a specimen becomes available. Inter-site movement (`#15`) is item transit, not a second hold type.

## One type

| Field | Role |
|---|---|
| `biblioId` | Queue unit. Always stored. |
| `itemId` | Allocation. `null` while waiting for any copy; set when staff pin a specimen or when fulfillment assigns one. |
| `pickupSiteId` | Lives on the hold itself (not a second reservation). Transit moves the allocated copy toward this site, then the hold becomes `ready`. |

`POST /api/v1/holds` with `biblioId` (OPAC / desk title hold) or `itemId` (staff pin a copy). Both create the same row type on the same biblio queue.

## Shelf capture (`to_pick`)

Placing a title hold (`biblioId` only) looks for an eligible shelf copy in the same transaction (`FOR UPDATE SKIP LOCKED`): on the shelf, borrowable (reception complete), not withdrawn, not on loan, not already reserved, not in transit. A copy at the pickup site is preferred.

- A copy is found: `itemId` is set and `status = to_pick`. The patron is **not** notified. The copy stays on the shelf until staff pull it.
- No copy is free: `itemId` stays null and `status = pending` (FIFO).

`GET /api/v1/holds/pick-list` (staff) lists `to_pick` rows. Optional `siteId` filters on the copy's current branch.

Staff check the copy in on the existing return scan (`POST /api/v1/loans/items/{barcode}/return`):

- Same site (or no pickup site): `status = ready`, usual notification.
- Pickup site differs: existing transit flow (`item_transits` `requested`, hold stays allocated and `pending` until receive marks it `ready`).

A returned copy is already in staff hands, so it is **not** `to_pick`. It follows the fulfillment rules below.

`POST /api/v1/holds/{id}/pick-not-found` (staff) means the copy was not on the shelf. The copy's `circulation_status` becomes `toSearch` (not lendable, not assignable). The hold is reassigned to another eligible shelf copy as `to_pick`, or returns to `pending` when none is free. A later scan of that barcode clears `toSearch` and runs the normal return path (`ready` or transit). Declaring the copy lost still uses the existing lost transition.

`circulation_settings.pick_delay_days` (`pickDelayDays` on `GET`/`PUT /holds/policy`, default 2) flags pick-list rows overdue. It does not expire or cancel the hold.

Checkout of a `to_pick` copy by another patron is `422` with code `item_to_pick_for_other` and `details.holdId`, `details.userId`, `details.cardNumber`, `details.initials`. The body never includes the patron name. `force=true` still releases the assignment.

## Who may borrow a shelf copy

A hold reserves only the copy it is assigned to (`to_pick`, `ready`, or a `pending` hold pinned to that `itemId`). An unassigned `pending` title hold does not make every copy unloanable. The queue takes the next returned copy. A copy already `to_pick` or `ready` for someone is reserved for them: checkout by another patron is the existing 422, overridable with `force=true`. Force releases that `to_pick` assignment back to `pending`, or onto another shelf copy as `to_pick`, instead of cancelling the patron's place.

## Fulfillment

When a copy becomes available (loan return, ready-hold cancel, ready-hold expiry):

1. If this copy already has a `ready` or `to_pick` hold, or an active transit, do nothing.
2. Otherwise offer the copy to the next `pending` hold on **that biblio** (position / `created_at` FIFO) that can take it:
   - unassigned (`itemId` null) — any copy
   - pinned to this specimen — this copy only
3. Same-site (no `pickupSiteId`, or it matches the copy's current `sourceId`): allocate, `status = ready`, `notifiedAt` / `expiresAt`.
4. Cross-site: allocate the copy, keep the hold `pending`, create an `item_transits` row (`requested`) toward **that hold’s** `pickupSiteId`. Staff ship → `in_transit` (not checkoutable). Receive at pickup marks the **same** hold `ready` and starts expiry. No parallel queue.

Cancel of a hold with an in-transit copy frees the reservation and opens a reverse transit back to the origin.

A hold pinned to a **different** copy is left in place; it does not block later patrons who can take this copy, and it is not a second queue.

Concurrent returns of two copies use `FOR UPDATE SKIP LOCKED` so the same hold cannot be notified twice.

## Uniqueness and the `#16` cap

- One active (`pending`/`to_pick`/`ready`) hold per patron per biblio.
- Every active hold counts as **one** slot toward `maxActiveHolds`.

## Listing

| Endpoint | Contents |
|---|---|
| `GET /biblios/:id/holds` | The notice-level queue (FIFO) |
| `GET /items/:id/holds` | Holds already allocated or pinned to that copy |
| `GET /holds` | All holds (staff) or the caller’s holds (`own`) |

`HoldDetails.biblio.items` is empty until a copy is allocated; afterwards it contains exactly that specimen.
