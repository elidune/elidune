# Serial gifts (one-off periodical copies)

**Status:** approved  
**Date:** 2026-11-02

## Goal

Centralize one-off periodical gifts under **Serials**, without creating a `serial_subscription`. A gift is one physical copy on a periodical biblio, tracked for staff history.

## Decisions

- No subscription for punctual gifts.
- Hub `/serials?tab=gifts` (Abonnements | Dons). Notice detail may deep-link with `biblioId` prefilled.
- Persistence: `serial_gifts` row + normal `items` row (`volume_designation` = issue label). Cover date lives on `serial_gifts`, not on `serial_issues`.
- Auth: `serials_rights` read/list, write/create (same as issue receipt: item insert via catalog, no separate `items_rights` on the gifts API).
- Optional new periodical: UI creates a minimal `periodic` biblio (same as subscription create), then posts the gift.

## API

- `GET /api/v1/serials/gifts` — recent gifts (q, siteId, page, perPage).
- `POST /api/v1/serials/gifts` — create gift copy + registry row.

## Out of scope

- Linking gifts to vendors/funds/orders.
- Auto-creating `serial_issues` for gifts.
- Retention rules for gift copies.
