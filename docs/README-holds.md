# Holds — copy-level and title-level

Elidune supports two hold scopes on the same `holds` row. There is no second reservation type for transfers; inter-site movement is a later item transit (`#15`), not another queue.

## Scopes

| Scope | Request | While `pending` | After fulfillment |
|---|---|---|---|
| **Copy-level** | `POST /api/v1/holds` with `itemId` | Queued on that barcode | `ready` on the same `itemId` |
| **Title-level** | `POST /api/v1/holds` with `biblioId` (no `itemId`) | `itemId` is `null`; queued on the bibliographic record | Next free copy is assigned (`itemId` set) and status becomes `ready` |

`biblioId` is always stored. `pickupSiteId` is accepted and stored as a nullable stub for `#15`; it is not used for routing yet.

Staff may still place a copy-level hold when a specific specimen is required. OPAC and desk both use the same `POST /holds` body.

## Fulfillment precedence

When a copy becomes available (loan return, ready-hold cancel, ready-hold expiry), `holds_notify_next` assigns **at most one** patron to that copy:

1. **Copy-level first** — if this specimen already has a `pending` hold, that queue wins (position order), even if an older title-level hold exists on the same biblio.
2. **Title-level FIFO** — otherwise the oldest unassigned title-level hold on that biblio is promoted: `item_id` is set to the returned copy, `status = ready`, `notified_at` / `expires_at` are set.

A `ready` hold already sitting on the copy blocks further promotion.

## Concurrency

Two copies of the same title can be returned at the same time. Title-level promotion uses `FOR UPDATE SKIP LOCKED` on the unassigned title queue so each return traps a **different** patron. The same title-level hold cannot be notified twice.

## Uniqueness and the `#16` cap

- One active (`pending`/`ready`) **copy-level** hold per patron + specimen.
- One active **title-level** hold per patron + biblio.
- A patron cannot hold both a title-level hold and any copy-level hold on the same biblio.
- Two copy-level holds on **different** copies of the same title remain allowed.
- Every active hold, including title-level, counts as **one** slot toward `maxActiveHolds` (`#16`).

## Queue listing

| Endpoint | Contents |
|---|---|
| `GET /items/:id/holds` | Active holds already assigned to that copy |
| `GET /biblios/:id/holds` | Active holds on the title: copy-level first, then title-level FIFO |
| `GET /holds` | All holds (staff) or the caller's holds (`own`) |

`HoldDetails.biblio.items` is empty while a title-level hold is unassigned; after promotion it contains exactly the trapped copy.
