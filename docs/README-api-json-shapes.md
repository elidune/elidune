# API JSON Shapes — Frontend Reference

All JSON request bodies and response objects use **camelCase** keys.  
This document lists every public type with its exact JSON field names.

**Conventions:**
- IDs are serialized as **strings** (Snowflake i64 encoded as JSON string)
- Dates/times are **ISO 8601** strings (`"2026-03-24T10:00:00Z"`)
- Optional fields are omitted when null unless noted
- Status enums that are **single-word** stay lowercase (e.g. `"pending"`, `"open"`)

---

## Auth (`/api/v1/auth`)

### `POST /auth/login`

Request body — `LoginRequest`:
```json
{ "username": "jdoe", "password": "secret", "deviceId": "uuid-optional" }
```

Response — `LoginResponse`:
```json
{
  "token": "eyJ...",
  "tokenType": "Bearer",
  "expiresIn": 86400,
  "user": { ...UserInfo... },
  "requires2fa": false,
  "twoFactorMethod": null,
  "deviceId": null,
  "mustChangePassword": false
}
```

### `UserInfo` (embedded in login response)
```json
{
  "id": "927364819265437697",
  "login": "jdoe",
  "email": "jdoe@example.com",
  "firstname": "Jean",
  "lastname": "Doe",
  "addrStreet": "12 rue de la Paix",
  "addrZipCode": 75001,
  "addrCity": "Paris",
  "phone": "+33612345678",
  "birthdate": "1990-01-15",
  "accountType": "librarian",
  "language": "french"
}
```

### `POST /auth/verify-2fa`

Request — `Verify2FARequest`:
```json
{ "userId": "927364819265437697", "code": "123456", "deviceId": null, "trustDevice": false }
```

Response — `Verify2FAResponse`:
```json
{ "token": "eyJ...", "tokenType": "Bearer", "expiresIn": 86400 }
```

### `POST /auth/verify-recovery`

Request — `VerifyRecoveryRequest`:
```json
{ "userId": "927364819265437697", "code": "ABCD-EFGH-IJKL" }
```

### `POST /auth/request-password-reset`

Request — `RequestPasswordResetRequest`:
```json
{ "identifier": "jdoe@example.com", "resetUrl": "https://app.example.com/reset?token=<token>" }
```

### `POST /auth/reset-password`

Request — `ResetPasswordRequest`:
```json
{ "token": "reset-token-from-email", "newPassword": "newSecret123" }
```

### `POST /auth/setup-2fa`

Request — `Setup2FARequest`:
```json
{ "method": "totp" }
```

Response — `Setup2FAResponse`:
```json
{ "provisioningUri": "otpauth://...", "recoveryCodes": ["ABCD-1234", ...] }
```

### `POST /auth/change-password`

Request — `ChangePasswordRequest`:
```json
{ "newPassword": "newSecret123" }
```

---

## Users (`/api/v1/users`)

### `User` (full user object)
```json
{
  "id": "927364819265437697",
  "groupId": null,
  "barcode": "LIB-0001",
  "login": "jdoe",
  "firstname": "Jean",
  "lastname": "Doe",
  "email": "jdoe@example.com",
  "addrStreet": "12 rue de la Paix",
  "addrZipCode": 75001,
  "addrCity": "Paris",
  "phone": "+33612345678",
  "birthdate": "1990-01-15",
  "createdAt": "2026-01-01T10:00:00Z",
  "updateAt": "2026-03-01T09:00:00Z",
  "issueAt": null,
  "accountType": "librarian",
  "fee": "free",
  "publicType": "818273645564928001",
  "notes": null,
  "status": 0,
  "archivedAt": null,
  "language": "french",
  "sex": null,
  "staffType": null,
  "hoursPerWeek": null,
  "staffStartDate": null,
  "staffEndDate": null,
  "twoFactorEnabled": false,
  "twoFactorMethod": null,
  "receiveReminders": true,
  "mustChangePassword": false
}
```

### `UserShort` (embedded in loans, holds, etc.)
```json
{
  "id": "927364819265437697",
  "firstname": "Jean",
  "lastname": "Doe",
  "accountType": "librarian",
  "publicType": "818273645564928001",
  "nbLoans": 3,
  "nbLateLoans": 0
}
```

### `UserQuery` (GET /users query params)
| Param | camelCase key |
|-------|---------------|
| `name` | `name` |
| `barcode` | `barcode` |
| `page` | `page` |
| `perPage` | `perPage` |

`GET /users` (including `?barcode=` and `?name=`) still requires `users_rights` read or write. A loans-only desk agent receives **403**. Name search is not a card scan.

### `GET /users/by-card/:card_number`

Exact `users.barcode` match for the circulation desk (one path segment, case-sensitive, not a prefix). Deleted patrons are **404**.

Allowed when `users_rights` is read or write, **or** `loans_rights` is read or write. `loans_rights` own and `users_rights` own do not qualify. Neither right is **403**.

`users_rights` read or write — the full `User` body (name, email, phone, address, `login`, `barcode`) plus `loanStatus`.

`loans_rights` read or write without users read — only:

```json
{
  "id": "927364819265437697",
  "cardNumber": "LIB-0001",
  "initials": "JD",
  "loanStatus": {
    "subscriptionValid": true,
    "subscriptionExpiresAt": null,
    "blocked": false,
    "blockReasons": [],
    "activeLoans": 0,
    "overdueLoans": 0,
    "maxLoans": 5,
    "unpaidFinesCents": 0,
    "readyHolds": 0
  }
}
```

No `firstname`, `lastname`, `email`, `phone`, address, or `login`. `cardNumber` is `users.barcode` and is null only when the stored card is blank. `initials` is the uppercase first letter of `firstname` plus the uppercase first letter of `lastname`; a missing or blank part is skipped.

`loanStatus` is the same object for both callers. `subscriptionExpiresAt` is the library-local `YYYY-MM-DD` of `users.expiry_at`, or null when the patron has no expiry. `subscriptionValid` is false when that instant is already past (same comparison as checkout). `maxLoans` is the audience total cap (`nb_max` on the public-type default loan row, else the global default, else 5). Null would mean no total cap; the resolver does not produce that today. `unpaidFinesCents` is the unpaid remainder in integer cents.

`blocked` is true when checkout **without** `force` would refuse this patron. `blockReasons` uses that same order, then overdue:

| Code | Sets `blocked` | Checkout without `force` |
|------|----------------|--------------------------|
| `account_blocked` | yes | `User::can_borrow` is false (not active, archived, or guest) |
| `card_lost` | yes | `users.card_lost` (card declared lost or stolen). Checkout and renewal are always **422** `{ "code": "card_lost", "error": "Business Rule Violation", "message": "Library card is reported lost or stolen" }`. `force=true` does not override, including for an admin. `POST /loans/batch-create` reports the same code on `results[].code`. `GET /me/loans` sets `canRenew` false and `renewalRefusalCode` to `card_lost`. Returning a copy and paying a fine stay allowed |
| `subscription_expired` | yes | `users.expiry_at` is before now |
| `unpaid_fines` | yes | unpaid balance is **strictly above** the configured threshold (public-type override, else `circulation_settings.unpaid_fine_threshold`). Not a hardcoded amount. Equal to the threshold does not block |
| `max_loans` | yes | active loans are at or above the total cap above. A per-media cap is enforced only when that item is checked out, so it is not a card-scan reason |
| `overdue_loans` | no | at least one unreturned loan is past `expiry_at`. Checkout does **not** refuse for this alone |

`readyHolds` is the number of this patron's holds in status `ready`. The calling agent has no site on the user row or the JWT, so the count includes every pickup site (including a null `pickup_site_id`). If an agent site is supplied later, the count is `holds.pickup_site_id` equal to that site.

409 checkout conflict bodies are unchanged.

### `UserPayload` (POST/PUT /users body)
```json
{
  "barcode": "LIB-0002",
  "login": "jsmith",
  "password": "secret",
  "firstname": "John",
  "lastname": "Smith",
  "email": "jsmith@example.com",
  "addrStreet": "5 avenue Victor Hugo",
  "addrZipCode": 69001,
  "addrCity": "Lyon",
  "phone": null,
  "birthdate": null,
  "accountType": "reader",
  "fee": "free",
  "publicType": "818273645564928001",
  "groupId": null,
  "sex": null,
  "staffType": null,
  "hoursPerWeek": null,
  "staffStartDate": null,
  "staffEndDate": null
}
```

### `UpdateProfile` (PATCH /auth/profile)
```json
{
  "firstname": "Jean-Marie",
  "email": "jm@example.com",
  "addrStreet": "...",
  "addrZipCode": 75001,
  "addrCity": "Paris",
  "currentPassword": "old",
  "newPassword": "new"
}
```

---

## Loans (`/api/v1/loans`)

### `CreateLoanRequest` (POST /loans body)
```json
{ "userId": "927364819265437697", "itemId": "818273645564928001", "itemIdentification": null, "force": false }
```

`itemId` or `itemIdentification` (barcode) identifies the copy. `force` bypasses patron account, subscription, loan-limit, unpaid-fine, transit, and hold-queue checks. It never overrides an open loan (reserved-item override is tracked in #167). It never overrides a lost or stolen card.

| Situation | HTTP | `code` |
|-----------|------|--------|
| Patron `users.card_lost` (`force` true or false, any role, including admin) | 422 | `card_lost` |
| Copy on loan to another borrower (`force` true or false, any role) | 409 | `item_on_loan_to_other_borrower` |
| Copy already on loan to this borrower | 409 | `item_already_on_loan_to_borrower` |

`card_lost` body is `{ "code": "card_lost", "error": "Business Rule Violation", "message": "Library card is reported lost or stolen" }`.

`POST /loans/:id/renew` (staff, and the patron renewing their own loan) and `POST /loans/items/:item_id/renew` return that same body when `users.card_lost` is set. `?force=true` does not override it, including for an admin. `GET /me/loans` reports `canRenew: false` and `renewalRefusalCode: "card_lost"`. There is no batch renewal route. Check-in (`POST /loans/:id/return` and `POST /loans/items/:barcode/return`) and `POST /fines/:id/pay` stay accepted for that patron.

The same-borrower `message` tells staff to renew the existing loan. The previous open loan stays open. A race on `idx_loans_one_active_per_item` returns the same 409 codes.

### `LoanResponse` (POST /loans response)
```json
{ "id": "927364819265437700", "issueAt": "2026-04-24T00:00:00Z", "message": "Loan created" }
```

### `GET /loans/:id/user`

`loans_rights` write may read any active loan. `loans_rights` own or read may read only the caller's own loan (a patron). Below own is 403. A loan that belongs to someone else is 403 when `loans_rights` is below write.

The body depends on `users_rights`, with one exception: the borrower receives their own `UserShortStaff` even when `users_rights` is only own. Loans write does not unlock another patron's name. `users_rights` own is not read for someone else's loan.

`users_rights` read or write — `UserShortStaff` (`barcode` is `users.barcode`):

```json
{
  "id": "927364819265437697",
  "login": "jdoe",
  "barcode": "LIB-0001",
  "firstname": "Jean",
  "lastname": "Doe",
  "accountType": "reader",
  "publicType": 1,
  "nbLoans": 1,
  "nbLateLoans": 0,
  "status": "active",
  "createdAt": "2026-03-24T10:00:00Z",
  "expiryAt": "2027-03-24T10:00:00Z"
}
```

Any other caller who may hit this route — `LoanBorrowerMinimized`. `cardNumber` is `users.barcode` and is `null` when the patron has no card. `initials` is the uppercase first letter of `firstname` plus the uppercase first letter of `lastname`; a missing or blank part is skipped.

```json
{ "id": "927364819265437697", "cardNumber": "LIB-0001", "initials": "JD" }
```

No other personal field (`firstname`, `lastname`, `login`, email, phone, address) is included in that shape.

### `LoanDetails` (embedded in return responses)

`POST /loans/:id/return`, `POST /loans/items/:barcode/return`, and `POST /loans/batch-return` (`results[].loan`) keep the field name `user`. Its shape follows the same rule as `GET /loans/:id/user`, except the caller is never treated as "the borrower" just because they checked the copy in: `users_rights` read or write receives `UserShortStaff`; otherwise `user` is only `{ id, cardNumber, initials }` (no `firstname`, `lastname`, `email`, or `login`).

```json
{
  "id": "927364819265437700",
  "itemId": "818273645564928001",
  "startDate": "2026-03-24T10:00:00Z",
  "expiryAt": "2026-04-24T00:00:00Z",
  "renewalDate": null,
  "nbRenews": 0,
  "returnedAt": "2026-03-30T10:00:00Z",
  "biblio": { "...": "BiblioShort" },
  "user": { "id": "927364819265437697", "cardNumber": "LIB-0001", "initials": "JD" },
  "itemIdentification": "978-2-07-040850-4",
  "isOverdue": false
}
```

With `users_rights` read or write, `user` is the `UserShortStaff` object shown under `GET /loans/:id/user` (`barcode`, not `cardNumber`).

### `ReturnResponse` (POST /loans/:id/return)
```json
{ "status": "returned", "loan": { ...LoanDetails... } }
```

When the copy is lost, the same response adds `wasLost: true` and, when a replacement charge is still open, `fineId`, `fineStatus` (`unpaid` | `paid` | `partial`), `fineAmount`, and `paidAmount` (integer cents). `status` is `lost_found` when the loan was already closed by the lost declaration.

### `ResolveLostFoundRequest` (POST /loans/items/:itemId/lost-found)
```json
{ "disposition": "damaged", "notes": null }
```

`disposition`: `circulate` | `damaged` | `leave`

### `LostFoundResolution`
```json
{
  "itemId": "818273645564928001",
  "itemStatus": "damaged",
  "borrowable": false,
  "disposition": "damaged",
  "fineAction": "unchanged",
  "fineId": "927364819265437702",
  "refundedAmount": 0,
  "cancelledAmount": 0
}
```

`fineAction`: `none` | `cancelled` | `refunded` | `partial` | `unchanged`. Amounts are integer cents. `fineId` is omitted when no open replacement charge is linked.

- `circulate` — copy becomes lendable and holds are served. An unpaid replacement charge is cancelled, a collected charge is refunded, and a partial payment is refunded with the remainder cancelled.
- `damaged` — item status `damaged`, not lendable. The replacement charge is left exactly as it is (`fineAction: "unchanged"`, both amounts `0`). No `fine.cancelled` or `fine.refunded` audit row is written. Partial relief is a separate explicit waive (`fine.waived`) with a reason.
- `leave` — copy stays lost and the charge is not touched (`fineAction: "none"`).

### Query params — `OverdueLoansQuery`
`?page=1&perPage=20`

### Query params — `SendRemindersQuery`
`?dryRun=true`

### `GET /users/:id/loans`

The caller may read their own loans. Another patron requires `users_rights` read (or write) and `loans_rights` read. Account type is not the gate.

Each row's `user` follows the users-rights rule. With users read, the row is `LoanDetailsStaff` (`user.login`, `user.barcode`, copy `weedingStatus`). Without users read, the row is `LoanDetailsOwn`: public copies, and `user` is only `{ id, cardNumber, initials }`. A patron therefore does not receive their own name or login on this list.

### Query params — `GetUserLoansQuery`
`?archived=false`

---

## Biblios & Items

### `CreateBiblioQuery` / `CreateItemQuery` (query params)
`?allowDuplicateIsbn=false&confirmReplaceExistingId=123`

### `ImportMarcBatchQuery` (query params for `POST /biblios/import-marc-batch`)
`?sourceId=100000000000000001&batchId=927364819265437696&recordId=1`

### MARC preview — `EnqueueResult` (`POST /biblios/load-marc`, `GET /biblios/marc-batch/:batchId`)

Réponse commune après upload UNIMARC ou rechargement d’un lot en cache. Voir **[MARC import preview — guide GUI](marc-import-preview-api-gui.md)** (migration `biblios` → `previews`, champ `validationIssues`).

```json
{
  "batchId": "927364819265437703",
  "previews": [
    {
      "id": "0",
      "mediaType": "b",
      "isbn": "978-2-07-040850-4",
      "title": "…",
      "date": "1995",
      "status": 0,
      "isValid": true,
      "archivedAt": null,
      "author": null,
      "items": [],
      "validationIssues": []
    }
  ]
}
```

Each preview item is a **`BiblioShort`-shaped object** (flattened) plus **`validationIssues`** (array from marc-rs `Record::validation_issues`).

### `CreateBiblioResponse` / `CreateItemResponse`
```json
{ "biblio": { ...Biblio... }, "importReport": { ...ImportReport... } }
```

### `ImportReport`
```json
{
  "action": "mergedBibliographic",
  "existingId": "123456789",
  "warnings": ["No ISBN found"],
  "message": "Merged into existing record"
}
```

`action` values: `created` | `mergedBibliographic` | `replacedArchived` | `replacedConfirmed`

### `DuplicateConfirmationRequired` (409 body)
```json
{
  "code": "duplicate_isbn_needs_confirmation",
  "existingId": "123456789",
  "existingBiblio": { ...BiblioShort... },
  "message": "A biblio with the same ISBN already exists"
}
```

### `DuplicateItemBarcodeRequired` (409 body)
```json
{
  "code": "duplicate_barcode_needs_confirmation",
  "existingId": "987654321",
  "existingItem": { ...ItemShort... },
  "message": "An item with this barcode already exists"
}
```

### `Biblio` (full bibliographic record)
```json
{
  "id": "927364819265437697",
  "mediaType": "b",
  "isbn": "978-2-07-040850-4",
  "title": "Sherlock Holmes",
  "subject": null,
  "audienceType": "adult",
  "lang": "french",
  "langOrig": null,
  "publicationDate": "1995",
  "pageExtent": "250",
  "format": null,
  "tableOfContents": null,
  "accompanyingMaterial": null,
  "abstract": null,
  "notes": null,
  "keywords": ["detective", "mystery"],
  "isValid": true,
  "seriesIds": [],
  "seriesVolumeNumbers": [],
  "editionId": null,
  "collectionIds": [],
  "collectionVolumeNumbers": [],
  "createdAt": "2026-01-01T10:00:00Z",
  "updatedAt": null,
  "archivedAt": null,
  "authors": [{ ...Author... }],
  "series": [],
  "collections": [],
  "edition": null,
  "items": []
}
```

`audienceType` values: `juvenile` | `preschool` | `primary` | `children` | `youngAdult` | `adultSerious` | `adult` | `general` | `specialized` | `unknown`

### `BiblioShort` (embedded in loans, tasks, etc.)
```json
{
  "id": "927364819265437697",
  "mediaType": "b",
  "isbn": "978-2-07-040850-4",
  "title": "Sherlock Holmes",
  "date": "1995",
  "status": 0,
  "isValid": true,
  "archivedAt": null,
  "author": { ...Author... },
  "items": [{ ...ItemShort... }]
}
```

### `Author` (embedded in Biblio.authors)
```json
{
  "id": "819283746556492801",
  "key": null,
  "lastname": "Conan Doyle",
  "firstname": "Arthur",
  "bio": null,
  "notes": null,
  "function": "author"
}
```

`function` values: `author` | `illustrator` | `translator` | `scientificAdvisor` | `prefaceWriter` | `photographer` | `publishingDirector` | `composer`

### `BiblioAuthor` (junction row in /biblio-authors)
```json
{
  "id": "100000000000000010",
  "biblioId": "927364819265437697",
  "authorId": "819283746556492801",
  "role": null,
  "authorType": 0,
  "position": 0
}
```

### `Serie` (embedded in Biblio.series)
```json
{
  "id": "200000000000000001",
  "key": null,
  "name": "Les Aventures",
  "issn": null,
  "createdAt": "2026-01-01T10:00:00Z",
  "updatedAt": null,
  "volumeNumber": 3
}
```

### `Collection` (embedded in Biblio.collections)
```json
{
  "id": "200000000000000002",
  "key": null,
  "name": "Bibliothèque de la Pléiade",
  "secondaryTitle": null,
  "tertiaryTitle": null,
  "issn": null,
  "createdAt": "2026-01-01T10:00:00Z",
  "updatedAt": null,
  "volumeNumber": null
}
```

### `Edition` (embedded in Biblio.edition)
```json
{
  "id": "200000000000000003",
  "publisherName": "Gallimard",
  "placeOfPublication": "Paris",
  "date": "1995",
  "createdAt": "2026-01-01T10:00:00Z",
  "updatedAt": null
}
```

### `Item` (physical copy / specimen)
```json
{
  "id": "818273645564928001",
  "biblioId": "927364819265437697",
  "sourceId": "100000000000000001",
  "barcode": "978-2-07-040850-4",
  "callNumber": "FIC DOY",
  "volumeDesignation": null,
  "place": null,
  "borrowable": true,
  "circulationStatus": null,
  "notes": null,
  "price": null,
  "createdAt": "2026-01-01T10:00:00Z",
  "updatedAt": null,
  "archivedAt": null,
  "sourceName": "Fonds général"
}
```

### `ItemShort`
```json
{ "id": "818273645564928001", "barcode": "978-2-07-040850-4", "callNumber": "FIC DOY", "borrowable": true, "sourceName": "Fonds général" }
```

### Copy availability on `GET /biblios/:id`

Each copy on the bibliographic notice includes:

| Field | Patron and OPAC (`ItemPublicView`) | Staff (`ItemStaffView`) |
|---|---|---|
| `dueDate` | Active loan `expiry_at`, or `null` | Same |
| `onHold` | Boolean, see below | Same |
| `loanId` | Omitted | Kept |
| `borrower` | Omitted | Only when the caller has `loans_rights` read or write and the copy is on loan |

`onHold` is not the `availability` value `onHold`. It is true when an active hold (`pending`, `to_pick`, or `ready`) is waiting:

- for **this copy**, when `holds.item_id` is this copy, or
- for **the title**, when `holds.item_id` is null and `holds.biblio_id` is this record. That unassigned title hold sets `onHold` on every copy, including a copy that is on loan. A hold already assigned to another copy does not.

`availability` still prefers `onLoan` over `onHold`, so a loaned copy can be `availability: "onLoan"` with `onHold: true` and a `dueDate`.

`borrower` uses the loan identity helper. `loans_rights` read or write without `users_rights` read returns `{ "id", "cardNumber", "initials" }`. `users_rights` read or write returns `UserShortStaff` (name, `login`, `barcode`). Users read without loans read omits `borrower`. Patrons and `GET /opac/biblios/:id` never receive `borrower`, `loanId`, or a hold patron.

`dueDate` and `onHold` are also on the other full-copy views (`GET /biblios/:id/items`, `GET /items/:id`, `GET /items/barcode/:barcode`, `GET /opac/biblios/:id`). Those routes do not add `borrower`. List hits (`ItemShort`) omit all four fields.

---

## Z39.50 (`/api/v1/z3950`)

### `Z3950SearchQuery` (query params)
`?query=doyle+sherlock&serverId=100000000000000002&maxResults=20`

### `Z3950SearchResponse`
```json
{ "total": 5, "biblios": [...], "source": "BnF Z39.50" }
```

### `Z3950ImportRequest`
```json
{
  "biblioId": "818273645564928001",
  "items": [{ "barcode": "978-2-07-040850-4", "callNumber": "FIC DOY", "sourceId": "100000000000000001" }],
  "confirmReplaceExistingId": null
}
```

### `ImportItem` (nested in Z3950ImportRequest)
```json
{ "barcode": "978-2-07-040850-4", "callNumber": "FIC DOY", "status": null, "place": null, "notes": null, "price": null, "sourceId": "100000000000000001" }
```

### `Z3950ImportResponse`
```json
{ "biblio": { ...Biblio... }, "importReport": { ...ImportReport... } }
```

---

## Sources (`/api/v1/sources`)

### `SourcesQuery` (query params)
`?includeArchived=false`

### `Source`
```json
{ "id": "100000000000000001", "key": "fonds-general", "name": "Fonds général", "isArchive": null, "archivedAt": null, "default": true }
```

### `MergeSources`
```json
{ "sourceIds": ["100000000000000001", "100000000000000002"], "name": "Fonds unifié" }
```

---

## Public Types (`/api/v1/public-types`)

### `PublicType`
```json
{
  "id": "818273645564928001",
  "name": "adult",
  "label": "Adulte",
  "subscriptionDurationDays": 365,
  "ageMin": 18,
  "ageMax": null,
  "subscriptionPrice": 1500,
  "maxLoans": 5,
  "loanDurationDays": 28
}
```

### `PublicTypeLoanSettings`
```json
{ "id": "...", "publicTypeId": "...", "mediaType": "b", "duration": 14, "nbMax": 3, "nbRenews": 1 }
```

### `UpsertLoanSettingRequest`
```json
{ "mediaType": "b", "duration": 14, "nbMax": 3, "nbRenews": 1 }
```

### `CreatePublicType` / `UpdatePublicType`
```json
{
  "name": "youth",
  "label": "Jeunesse",
  "subscriptionDurationDays": 365,
  "ageMin": null,
  "ageMax": 18,
  "subscriptionPrice": 500,
  "maxLoans": 5,
  "loanDurationDays": 14
}
```

---

## Holds (`/api/v1/holds`)

List endpoints (`GET /holds`, `GET /items/:id/holds`, `GET /biblios/:id/holds`, `GET /users/:id/holds`) return **`HoldDetails`**. Create/cancel responses use plain **`Hold`** (ids only, no embedded item/user).

See [README-holds.md](README-holds.md): one hold type, queue at biblio/notice, copy allocated later.

### `Hold`
```json
{
  "id": "927364819265437701",
  "userId": "927364819265437697",
  "biblioId": "718273645564928000",
  "itemId": "818273645564928001",
  "pickupSiteId": null,
  "createdAt": "2026-03-24T10:00:00Z",
  "notifiedAt": null,
  "expiresAt": null,
  "status": "pending",
  "position": 1,
  "notes": null
}
```

`itemId` is `null` for an unassigned title-level hold. `status` values: `pending` | `ready` | `fulfilled` | `cancelled` | `expired`

### `HoldDetails`
```json
{
  "id": "927364819265437701",
  "biblioId": "718273645564928000",
  "itemId": null,
  "biblio": { ...BiblioShort... },
  "user": { ...UserShort... },
  "createdAt": "2026-03-24T10:00:00Z",
  "notifiedAt": null,
  "expiresAt": null,
  "status": "pending",
  "position": 1,
  "notes": null
}
```

`biblio.items` is empty while the hold is title-level and unassigned; after a copy is trapped it has exactly **one** `ItemShort`.

### `CreateHold`
```json
{ "userId": "927364819265437697", "itemId": "818273645564928001", "notes": null }
```

Title-level (any copy of the bibliographic record):

```json
{ "userId": "927364819265437697", "biblioId": "718273645564928000", "notes": null }
```

Exactly one of `itemId` / `biblioId` is required. `pickupSiteId` is optional (transit hook, `#15`).

---

## Fines (`/api/v1/fines`)

### `Fine`
```json
{
  "id": "927364819265437702",
  "loanId": "927364819265437700",
  "userId": "927364819265437697",
  "amount": "3.50",
  "paidAmount": "0.00",
  "createdAt": "2026-03-24T10:00:00Z",
  "paidAt": null,
  "status": "pending",
  "notes": null
}
```

`status` values: `pending` | `partial` | `paid` | `waived`

### `FineRule`
```json
{ "id": 1, "mediaType": "b", "dailyRate": "0.10", "maxAmount": "5.00", "graceDays": 3, "notes": null }
```

### `UpsertFineRuleRequest` (PUT /fines/rules body)
```json
{ "mediaType": "b", "dailyRate": "0.10", "maxAmount": "5.00", "graceDays": 3 }
```

### `UnpaidFinesSummary` (GET /users/:id/fines response)
```json
{ "totalUnpaid": "3.50", "fines": [...Fine...] }
```

### `CirculationFinePolicy` (GET/PUT /fines/policy)
```json
{ "unpaidFineThreshold": "10.00" }
```

Global unpaid-balance gate for **checkout** and **renew**. Default `0` blocks any positive unpaid remainder. Paid and waived fines do not count. A public type may set `unpaidFineThreshold` to override this value for that audience.

When the patron is over the threshold, `POST /loans` and `POST /loans/:id/renew` return **422** with a desk-facing message (amount due + that staff can retry with `force=true`). A successful force override writes audit event `loan.fine_threshold_overridden` (`actor`, `unpaid`, `threshold`, `operation`).

### Accrue (staff)

| Method | Path | Body | Response |
|--------|------|------|----------|
| `POST` | `/loans/:id/fines/accrue` | — | `AccrueLoanResult` |
| `POST` | `/users/:id/fines/accrue` | — | `AccrueBatchReport` |
| `POST` | `/fines/accrue` | `{ "userId": "…" }` optional | `AccrueBatchReport` |

Policy: **one open (`pending`/`partial`) fine per loan**. Repeating accrue recalculates `amount` from overdue days vs the rule (grace, daily rate, cap). It does not insert a second open row.

`outcome` values: `created` | `updated` | `unchanged` | `skippedGrace` | `skippedNotOverdue` | `skippedReturned` | `skippedDeletedUser`

Single-loan accrue returns **422** for skip outcomes (grace, not overdue, returned, deleted patron). Batch endpoints always return **200** and count skips.

### `AccrueLoanResult`
```json
{
  "outcome": "created",
  "loanId": "927364819265437700",
  "userId": "927364819265437697",
  "fine": { "id": "927364819265437702", "amount": "3.50", "status": "pending" },
  "breakdown": {
    "overdueDays": 10,
    "graceDays": 3,
    "billableDays": 7,
    "dailyRate": "0.50",
    "maxAmount": "10.00",
    "amount": "3.50",
    "capped": false,
    "mediaType": "printedText"
  }
}
```

### `AccrueBatchReport`
```json
{ "created": 1, "updated": 0, "unchanged": 0, "skipped": 0, "errors": [], "results": [...AccrueLoanResult...] }
```

A scheduled job (`reminders.accrue_fines`, same `send_time` as overdue reminders) runs the all-overdue batch and writes `system.fines_accrual_batch_completed`.

---

## Inventory (`/api/v1/inventory`)

See **[Inventory API — frontend guide](inventory-api-frontend.md)** for semantics, query params, and error codes.

| Method | Path | Response |
|--------|------|----------|
| `GET` | `/inventory/sessions` | `PaginatedResponse<InventorySession>` (`page`, `perPage`, …) |
| `POST` | `/inventory/sessions` | `InventorySession` (**201**) |
| `GET` | `/inventory/sessions/:id` | `InventorySession` |
| `POST` | `/inventory/sessions/:id/close` | `InventorySession` |
| `POST` | `/inventory/sessions/:id/scan` | `InventoryScan` (**201**) |
| `POST` | `/inventory/sessions/:id/scans/batch` | `InventoryScan[]` (**201**), max **500** barcodes |
| `GET` | `/inventory/sessions/:id/scans` | `PaginatedResponse<InventoryScan>` |
| `GET` | `/inventory/sessions/:id/missing` | `PaginatedResponse<InventoryMissingRow>` |
| `GET` | `/inventory/sessions/:id/report` | `InventoryReport` |

### `InventorySession`
```json
{
  "id": "927364819265437703",
  "name": "Inventaire printemps 2026",
  "startedAt": "2026-03-24T08:00:00Z",
  "closedAt": null,
  "status": "open",
  "locationFilter": "Salle A",
  "notes": null,
  "scopePlace": 3,
  "createdBy": "927364819265437697"
}
```

`status` values: `open` | `closed`. `scopePlace` is **`null`** when the session covers all active items.

### `CreateInventorySession` (`POST /inventory/sessions`)
```json
{ "name": "Inventaire printemps 2026", "locationFilter": "Salle A", "notes": null, "scopePlace": 3 }
```

### `ScanBarcode` (`POST /inventory/sessions/:id/scan`)
```json
{ "barcode": "978-2-07-040850-4" }
```

### `BatchScanBarcodes` (`POST /inventory/sessions/:id/scans/batch`)
```json
{ "barcodes": ["978-2-07-040850-4", "9782123456789"] }
```

### `InventoryScan`
```json
{
  "id": 42,
  "sessionId": "927364819265437703",
  "itemId": "818273645564928001",
  "barcode": "978-2-07-040850-4",
  "scannedAt": "2026-03-24T09:00:00Z",
  "result": "found",
  "scannedBy": "927364819265437697"
}
```

`id` is a JSON **number**. `itemId` is **null** only for `unknown_barcode`.

`result` values: `found` | `found_archived` | `unknown_barcode`

### `InventoryMissingRow`
```json
{
  "itemId": "818273645564928002",
  "barcode": "9782123456789",
  "callNumber": "PQ 1234",
  "place": 3,
  "biblioTitle": "Example title"
}
```

### `InventoryReport`
```json
{
  "sessionId": "927364819265437703",
  "expectedInScope": 1200,
  "totalScanned": 320,
  "totalFound": 300,
  "totalFoundArchived": 5,
  "totalUnknown": 15,
  "distinctItemsScanned": 290,
  "duplicateScanCount": 15,
  "missingCount": 880,
  "missingScannable": 870,
  "missingWithoutBarcode": 10
}
```

`sessionId` is a JSON **string** (Snowflake), consistent with other inventory ids.

---

## Events (`/api/v1/events`)

### `Event`
List responses omit **`attachmentDataBase64`**; **`GET /events/:id`**, **`POST /events`**, and **`PUT /events/:id`** include it when an attachment is stored.

```json
{
  "id": "927364819265437704",
  "name": "Heure du conte",
  "eventType": 0,
  "eventDate": "2026-04-10",
  "startTime": "10:00",
  "endTime": "11:00",
  "attendeesCount": 15,
  "targetPublic": 106,
  "schoolName": null,
  "className": null,
  "studentsCount": null,
  "partnerName": null,
  "description": null,
  "notes": null,
  "createdAt": "2026-03-01T10:00:00Z",
  "updateAt": null,
  "announcementSentAt": null,
  "attachmentFileName": "flyer.pdf",
  "attachmentMimeType": "application/pdf",
  "attachmentSize": 48210,
  "attachmentDataBase64": "JVBERi0xLjQK…"
}
```

**`CreateEvent` / `UpdateEvent` attachment input** (optional): `{ "fileName": "…", "mimeType": "application/pdf", "dataBase64": "…" }`. **`UpdateEvent`** may set **`removeAttachment`: `true`** to clear the file. See **`docs/events-api-frontend.md`**.

### `EventsListResponse`
```json
{ "events": [...Event...], "total": 42 }
```

### `EventQuery` (query params)
`?startDate=2026-01-01&endDate=2026-12-31&eventType=0&page=1&perPage=20`

### `EventAnnualStats`
```json
{
  "totalEvents": 48,
  "totalAttendees": 520,
  "schoolVisits": 12,
  "distinctClasses": 8,
  "totalStudents": 240,
  "byType": [{ "eventType": 0, "count": 24, "attendees": 360 }]
}
```

---

## Schedules (`/api/v1/schedules`)

### `SchedulePeriod`
```json
{ "id": "...", "name": "Horaires hiver 2026", "startDate": "2026-01-01", "endDate": "2026-03-31", "notes": null, "createdAt": "...", "updateAt": null }
```

### `ScheduleSlot`
```json
{ "id": "...", "periodId": "...", "dayOfWeek": 1, "openTime": "09:00", "closeTime": "18:00", "createdAt": "..." }
```

### `ScheduleClosure`
```json
{ "id": "...", "closureDate": "2026-01-01", "reason": "Jour férié", "createdAt": "..." }
```

### `ScheduleClosureQuery` (query params)
`?startDate=2026-01-01&endDate=2026-12-31`

---

## Visitor Counts (`/api/v1/visitor-counts`)

### `VisitorCount`
```json
{ "id": "...", "countDate": "2026-03-24", "count": 87, "source": "manual", "notes": null, "createdAt": "..." }
```

### `VisitorCountQuery` (query params)
`?startDate=2026-01-01&endDate=2026-12-31`

---

## Equipment (`/api/v1/equipment`)

### `Equipment`
```json
{
  "id": "...",
  "name": "PC Multimédia 1",
  "equipmentType": 0,
  "hasInternet": true,
  "isPublic": true,
  "quantity": 1,
  "status": 0,
  "notes": null,
  "createdAt": "...",
  "updateAt": null
}
```

---

## Settings (`/api/v1/settings`)

### `SettingsResponse`
```json
{
  "loanSettings": [
    { "mediaType": "b", "maxLoans": 5, "maxRenewals": 2, "durationDays": 28 }
  ],
  "z3950Servers": [
    { "id": "...", "name": "BnF", "address": "z3950.bnf.fr", "port": 2211, "database": "TOUT", "format": "UNIMARC", "login": null, "password": null, "encoding": "utf-8", "isActive": true }
  ]
}
```

---

## Library Info (`/api/v1/library-info`)

### `LibraryInfo`
```json
{
  "name": "Médiathèque Municipale",
  "addrLine1": "1 place de l'Hôtel de Ville",
  "addrLine2": null,
  "addrPostcode": "75001",
  "addrCity": "Paris",
  "addrCountry": "France",
  "phones": ["+33 1 23 45 67 89"],
  "email": "mediatheque@mairie.fr",
  "timezone": "Europe/Paris",
  "language": null,
  "updatedAt": "2026-01-15T10:00:00Z"
}
```

`language` is the library default UI language: `"fr"`, `"en"`, `"de"`, `"es"`, or `null` when unset. Clients fall back to `fr` only when it is `null`. `PUT /library-info` (admin write-settings) omits the field to leave it unchanged and sends JSON `null` to clear it. Any other value is `400` `validation_error`.

---

## Admin Config (`/api/v1/admin/config`)

### `ConfigResponse`
```json
{
  "sections": [
    { "key": "email", "value": { ...json... }, "overridden": false, "overridable": true }
  ]
}
```

Overridable keys include `email`, `logging`, `reminders`, `audit`, `holds`, and `circulation`.
`circulation.skip_closed_days` (default `true`) pushes checkout and renew due dates to the next open day using `schedule_slots` and `schedule_closures`.

### `reminders` section (`GET/PUT/DELETE /admin/config/reminders`)

Snake_case keys (same as the TOML file). `PUT` replaces the whole section. A disabled tier turns off every later tier on write (no holes).

```json
{
  "enabled": true,
  "frequency_days": 7,
  "first_reminder_delay_days": 7,
  "second_reminder_delay_days": 14,
  "formal_notice_delay_days": 21,
  "first_reminder_template": "overdue_reminder",
  "second_reminder_template": "overdue_second_reminder",
  "formal_notice_template": "overdue_formal_notice",
  "first_reminder_enabled": true,
  "second_reminder_enabled": true,
  "formal_notice_enabled": true,
  "send_time": "09:00",
  "accrue_fines": true,
  "smtp_throttle_ms": 100,
  "overridable": true
}
```

Tier delays are days after the due date. Templates are ids from `GET /settings/email-templates`.
`frequency_days` is kept so existing payloads still deserialize; sending uses the three tier delays.

### `ReindexSearchResponse`
```json
{ "itemsQueued": 1250, "meilisearchAvailable": true }
```

---

## Statistics (`/api/v1/stats`)

### `StatsQuery` (query params)
`?year=2026&startDate=2026-01-01&endDate=2026-12-31&publicType=adult&mediaType=b`

### `StatsResponse`
```json
{
  "items": {
    "total": 8420,
    "byMediaType": [{ "label": "b", "value": 7200 }],
    "byPublicType": [{ "label": "adult", "value": 5000 }],
    "acquisitions": 320,
    "acquisitionsByMediaType": [],
    "withdrawals": 45,
    "withdrawalsByMediaType": []
  },
  "users": {
    "total": 1250,
    "active": 480,
    "byAccountType": [{ "label": "reader", "value": 1100 }]
  },
  "loans": {
    "active": 210,
    "overdue": 18,
    "returnedToday": 5,
    "byMediaType": [{ "label": "b", "value": 180 }]
  }
}
```

### `LoanStatsQuery` (query params — `GET /stats/loans`)
`?startDate=2026-01-01&endDate=2026-12-31&interval=month&mediaType=b&publicType=adult&userId=927364819265437697`

`interval` values: `day` | `week` | `month` | `year`

### `LoanStatsResponse`
```json
{
  "totalLoans": 3820,
  "totalReturns": 3750,
  "timeSeries": [{ "period": "2026-01", "loans": 320, "returns": 310 }],
  "byMediaType": [{ "label": "b", "value": 3100 }]
}
```

### `UserStatsQuery` (query params — `GET /stats/users`)
`?startDate=2026-01-01&endDate=2026-12-31`

`mode`, `sortBy`, and `limit` are ignored. The response is always the aggregate object below. There is no named borrower leaderboard, for any rights.

### `UserStatsAggregate` (`GET /stats/users`)
```json
{
  "usersTotal": 1250,
  "usersByPublicType": [{ "label": "adult", "value": 900 }],
  "usersByAgeBand": [{ "label": "30-49", "value": 400 }, { "label": "unknown", "value": 50 }],
  "usersByAccountType": [{ "label": "reader", "value": 1100 }],
  "usersBySex": [{ "label": "female", "value": 640 }],
  "newUsersTotal": 85,
  "newUsersByPublicType": [],
  "newUsersBySex": [],
  "activeBorrowersTotal": 480,
  "activeBorrowersByPublicType": [],
  "groupsTotal": 12
}
```

`usersByAgeBand` labels are `0-17`, `18-29`, `30-49`, `50-64`, `65+`, and `unknown` (no birthdate). `usersByAccountType` labels are account-type codes. Patrons are not assigned to a library site, so this body has no site breakdown. No field contains a patron name, login, email, or user id.

### `CatalogStatsQuery` (query params — `GET /stats/catalog`)
`?startDate=2026-01-01&endDate=2026-12-31&bySource=true&byMediaType=true&byPublicType=false`

### `CatalogStatsResponse`
```json
{
  "totals": { "activeItems": 8420, "enteredItems": 320, "archivedItems": 45, "loans": 3820 },
  "bySource": [
    { "sourceId": "...", "sourceName": "Fonds général", "activeItems": 7200, "enteredItems": 280, "archivedItems": 40, "loans": 3200, "byMediaType": null, "byPublicType": null }
  ],
  "byMediaType": null,
  "byPublicType": null
}
```

---

## Maintenance (`/api/v1/maintenance`)

### `MaintenanceRequest`
```json
{ "actions": ["cleanupSeries", "mergeDuplicateSeries", "cleanupOrphanAuthors"] }
```

Available `action` values:
| Value | Description |
|-------|-------------|
| `cleanupSeries` | Strip quotes from series names, delete orphan series |
| `cleanupCollections` | Strip quotes from collection names, delete orphan collections |
| `cleanupOrphanAuthors` | Delete authors not linked to any biblio |
| `mergeDuplicateSeries` | Merge series with identical names (case-insensitive) |
| `mergeDuplicateCollections` | Merge collections with identical names |
| `cleanupDanglingBiblioSeries` | Remove broken biblio↔series links |
| `cleanupDanglingBiblioCollections` | Remove broken biblio↔collection links |

Returns `202 Accepted` — poll `GET /tasks/:id` (see background tasks doc).

### `MaintenanceResponse` (task `result` when completed)
```json
{
  "reports": [
    { "action": "cleanupSeries", "success": true, "details": { "deleted": 12, "quotesStripped": 8 }, "error": null },
    { "action": "mergeDuplicateSeries", "success": false, "details": {}, "error": "timeout" }
  ]
}
```

---

## Background Tasks (`/api/v1/tasks`)

See [README-background-tasks.md](README-background-tasks.md) for full polling guide.

### `TaskAcceptedResponse` (202 on async endpoints)
```json
{ "taskId": "927364819265437705" }
```

### `BackgroundTask` (GET /tasks/:id)
```json
{
  "id": "927364819265437705",
  "kind": "marcBatchImport",
  "status": "running",
  "progress": { "current": 42, "total": 150, "message": "Importing record 42/150" },
  "result": null,
  "error": null,
  "createdAt": "2026-03-24T10:00:00Z",
  "startedAt": "2026-03-24T10:00:01Z",
  "completedAt": null,
  "userId": "927364819265437697"
}
```

`kind` values: `marcBatchImport` | `maintenance`  
`status` values: `pending` | `running` | `completed` | `failed`

### `MarcBatchImportReport` (task `result` when kind=`marcBatchImport`)
```json
{
  "batchId": "927364819265437696",
  "imported": ["1", "2", "5"],
  "failed": [{ "key": "3", "error": "Duplicate ISBN", "existingId": "42" }]
}
```

### MARC batch info — `MarcBatchInfo`
```json
{ "batchId": "927364819265437696", "recordCount": 150, "ttlSeconds": 3600 }
```

---

## Batch Operations (`/api/v1/loans/batch-*`)

### `BatchReturnRequest` (POST /loans/batch-return)
```json
{ "barcodes": ["978-2-07-040850-4", "978-2-07-040851-1"] }
```

### `BatchReturnResponse`
```json
{
  "returned": 2,
  "errors": 0,
  "results": [
    { "barcode": "978-2-07-040850-4", "loan": { ...LoanDetails... }, "success": true },
    { "barcode": "978-2-07-040851-1", "success": false, "error": "No active loan for this barcode" }
  ]
}
```

### `BatchCreateLoansRequest` (POST /loans/batch-create)
```json
{ "userId": "927364819265437697", "barcodes": ["978-2-07-040850-4"], "force": false }
```

### `BatchCreateLoansResponse`
```json
{
  "created": 1,
  "errors": 1,
  "results": [
    { "barcode": "978-2-07-040850-4", "success": true, "loanId": "927364819265437700" },
    {
      "barcode": "978-2-07-040851-1",
      "success": false,
      "error": "This item is on loan to another borrower.",
      "code": "item_on_loan_to_other_borrower"
    }
  ]
}
```

`error` is the same human-readable string as before. `code` is added only on failures and uses the same values as `ErrorResponse.code` (`item_on_loan_to_other_borrower`, `item_already_on_loan_to_borrower`, `card_lost`, `not_found`, `business_rule_violation`, …). A lost card is `card_lost` on that item even when the request sets `force: true`. Successful items omit both.

---

## History (`/api/v1/users/:id/history`)

### `HistoryPreference`
```json
{ "userId": "927364819265437697", "historyEnabled": true }
```

### `UpdateHistoryPreference` (PUT /users/:id/history/preference)
```json
{ "enabled": false }
```

---

## Audit Log (`/api/v1/audit`)

### `AuditLogEntry`
```json
{
  "id": 1234,
  "eventType": "loan.created",
  "userId": "927364819265437697",
  "entityType": "loan",
  "entityId": "927364819265437700",
  "ipAddress": "192.168.1.1",
  "payload": { "itemId": "818273645564928001" },
  "createdAt": "2026-03-24T10:00:00Z"
}
```

### `AuditQueryRequest` (query params for GET /audit)
`?eventType=loan.created&entityType=loan&entityId=123&userId=456&fromDate=2026-01-01T00:00:00Z&toDate=2026-12-31T23:59:59Z&page=1&perPage=50`

### `AuditExportRequest` (query params for GET /audit/export)
`?format=csv&eventType=loan.created&fromDate=2026-01-01T00:00:00Z&toDate=2026-12-31T23:59:59Z`

### `AuditLogPage`
```json
{ "entries": [...AuditLogEntry...], "total": 420, "page": 1, "perPage": 50 }
```

---

## Reminders

### `ReminderReport` (response to `POST /loans/send-overdue-reminders`)
```json
{
  "dryRun": false,
  "emailsSent": 18,
  "loansReminded": 18,
  "details": [
    {
      "userId": "...",
      "email": "jdoe@example.com",
      "firstname": "Jean",
      "lastname": "Doe",
      "loanCount": 2,
      "tier": "first",
      "templateId": "overdue_reminder"
    }
  ],
  "errors": [
    { "userId": "...", "email": "noreply@example.com", "errorMessage": "Invalid email address" }
  ]
}
```

`tier` is the notice that was queued (or would be queued on `dryRun=true`): `first` | `second` | `formalNotice`. Same-tier loans for one patron share one mail.

### `OverdueLoanView` (entries in GET /loans/overdue)

Borrower identity is only the nested `user` object.

`users_rights` read or write — full contact, including phone:

```json
{
  "loanId": "927364819265437700",
  "user": {
    "id": "927364819265437697",
    "firstname": "Jean",
    "lastname": "Doe",
    "email": "jdoe@example.com",
    "phone": "0612345678"
  },
  "biblioId": "818273645564928000",
  "title": "Les Misérables",
  "authors": "Victor Hugo",
  "itemBarcode": "978-2-07-123456-7",
  "loanDate": "2026-02-01T10:00:00Z",
  "expiryAt": "2026-03-01T10:00:00Z",
  "lastReminderSentAt": null,
  "reminderCount": 0,
  "nextReminderTier": "first"
}
```

Otherwise `user` is exactly `{ id, cardNumber, initials }`. `email`, `phone`, `firstname`, and `lastname` are absent.

```json
{ "id": "927364819265437697", "cardNumber": "LIB-0001", "initials": "JD" }
```

`nextReminderTier` is `first` | `second` | `formalNotice` | `null`. `null` means sending has stopped (formal notice already sent, highest enabled tier already sent, or the item is lost / claimed-returned). The send action uses this field to show which tier is next; it does not skip a disabled or already-sent tier.

### `OverdueLoansPage`
```json
{ "loans": [...OverdueLoanView...], "total": 18, "page": 1, "perPage": 20 }
```

---

## Error Responses

All error responses share the same envelope:
```json
{ "code": "not_found", "error": "Not Found", "message": "Biblio 123 not found" }
```

Common `code` values:
| code | HTTP status |
|------|------------|
| `authentication` | 401 |
| `authorization` | 403 |
| `not_found` | 404 |
| `validation_error` | 400 |
| `conflict` | 409 |
| `item_on_loan_to_other_borrower` | 409 |
| `item_already_on_loan_to_borrower` | 409 |
| `business_rule_violation` | 422 |
| `card_lost` | 422 |
| `internal_error` | 500 |

---

## TypeScript Type Definitions

```typescript
// ── Core ID type ──────────────────────────────────────────────
type ID = string; // Snowflake i64 serialized as string

// ── Enums ─────────────────────────────────────────────────────
type Language     = 'french' | 'english' | 'german' | 'spanish' | 'portuguese' | 'japanese';
type AccountType  = 'admin' | 'librarian' | 'reader' | 'guest';
type FeeType      = 'free' | 'local' | 'foreigner';
type FineStatus   = 'pending' | 'partial' | 'paid' | 'waived';
type HoldStatus = 'pending' | 'ready' | 'fulfilled' | 'cancelled' | 'expired';
type InventoryStatus   = 'open' | 'closed';
type TaskKind     = 'marcBatchImport' | 'maintenance';
type TaskStatus   = 'pending' | 'running' | 'completed' | 'failed';
type ImportAction = 'created' | 'mergedBibliographic' | 'replacedArchived' | 'replacedConfirmed';
type Interval     = 'day' | 'week' | 'month' | 'year';
type MaintenanceAction =
  | 'cleanupSeries' | 'cleanupCollections' | 'cleanupOrphanAuthors'
  | 'mergeDuplicateSeries' | 'mergeDuplicateCollections'
  | 'cleanupDanglingBiblioSeries' | 'cleanupDanglingBiblioCollections';
type AuthorFunction =
  | 'author' | 'illustrator' | 'translator' | 'scientificAdvisor'
  | 'prefaceWriter' | 'photographer' | 'publishingDirector' | 'composer';
type AudienceType =
  | 'juvenile' | 'preschool' | 'primary' | 'children' | 'youngAdult'
  | 'adultSerious' | 'adult' | 'general' | 'specialized' | 'unknown';

// ── Users ─────────────────────────────────────────────────────
interface UserShort {
  id: ID; firstname: string | null; lastname: string | null;
  accountType: AccountType | null; publicType: ID | null;
  nbLoans: number | null; nbLateLoans: number | null;
}

// ── Biblios ───────────────────────────────────────────────────
interface Author {
  id: ID; key: string | null; lastname: string | null; firstname: string | null;
  bio: string | null; notes: string | null; function: AuthorFunction | null;
}
interface BiblioAuthor {
  id: ID; biblioId: ID; authorId: ID;
  role: string | null; authorType: number; position: number;
}
interface Serie {
  id: ID | null; key: string | null; name: string | null; issn: string | null;
  createdAt: string | null; updatedAt: string | null; volumeNumber: number | null;
}
interface Collection {
  id: ID | null; key: string | null; name: string | null;
  secondaryTitle: string | null; tertiaryTitle: string | null; issn: string | null;
  createdAt: string | null; updatedAt: string | null; volumeNumber: number | null;
}
interface Edition {
  id: ID | null; publisherName: string | null; placeOfPublication: string | null;
  date: string | null; createdAt: string | null; updatedAt: string | null;
}
interface ItemShort {
  id: ID; barcode: string | null; callNumber: string | null;
  borrowable: boolean; sourceName: string | null;
}
interface BiblioShort {
  id: ID; mediaType: string; isbn: string | null; title: string | null;
  date: string | null; status: number; isValid: boolean | null;
  archivedAt: string | null; author: Author | null; items: ItemShort[];
}
interface Biblio {
  id: ID | null; mediaType: string; isbn: string | null; title: string | null;
  subject: string | null; audienceType: AudienceType | null;
  lang: string | null; langOrig: string | null;
  publicationDate: string | null; pageExtent: string | null;
  format: string | null; tableOfContents: string | null;
  accompanyingMaterial: string | null; abstract: string | null;
  notes: string | null; keywords: string[] | null; isValid: boolean | null;
  seriesIds: ID[]; seriesVolumeNumbers: (number | null)[];
  editionId: ID | null;
  collectionIds: ID[]; collectionVolumeNumbers: (number | null)[];
  createdAt: string | null; updatedAt: string | null; archivedAt: string | null;
  authors: Author[]; series: Serie[]; collections: Collection[];
  edition: Edition | null; items: Item[];
}

// ── Loans ─────────────────────────────────────────────────────
interface LoanDetails {
  id: ID; startDate: string; issueAt: string;
  renewalDate: string | null; nbRenews: number;
  returnedAt: string | null; biblio: BiblioShort;
  user: UserShort | null; itemIdentification: string | null;
  isOverdue: boolean;
}
type LostFoundDisposition = 'circulate' | 'damaged' | 'leave';
type LostFineAction = 'none' | 'cancelled' | 'refunded' | 'partial' | 'unchanged';
interface LostFoundResolution {
  itemId: ID; itemStatus: string; borrowable: boolean;
  disposition: LostFoundDisposition; fineAction: LostFineAction;
  fineId?: ID; refundedAmount: number; cancelledAmount: number;
}

// ── Fines ─────────────────────────────────────────────────────
interface Fine {
  id: ID; loanId: ID; userId: ID;
  amount: string; paidAmount: string;
  createdAt: string; paidAt: string | null;
  status: FineStatus; notes: string | null;
}
interface FineRule {
  id: number; mediaType: string | null; dailyRate: string;
  maxAmount: string | null; graceDays: number; notes: string | null;
}
interface UnpaidFinesSummary { totalUnpaid: string; fines: Fine[]; }

// ── Holds ─────────────────────────────────────────────────────
interface Hold {
  id: ID; userId: ID; itemId: ID;
  createdAt: string; notifiedAt: string | null; expiresAt: string | null;
  status: HoldStatus; position: number; notes: string | null;
}
interface HoldDetails {
  id: ID;
  biblio: BiblioShort;
  user: UserShort | null;
  createdAt: string; notifiedAt: string | null; expiresAt: string | null;
  status: HoldStatus; position: number; notes: string | null;
}

// ── Batch ─────────────────────────────────────────────────────
interface BatchReturnItemResult {
  barcode: string; loan?: LoanDetails | null; success: boolean; error?: string;
}
interface BatchReturnResponse { returned: number; errors: number; results: BatchReturnItemResult[]; }
interface BatchCreateLoanItemResult {
  barcode: string; success: boolean; loanId?: string | null; error?: string; code?: string;
}
interface BatchCreateLoansResponse { created: number; errors: number; results: BatchCreateLoanItemResult[]; }

// ── History ───────────────────────────────────────────────────
interface HistoryPreference { userId: ID; historyEnabled: boolean; }

// ── Tasks ─────────────────────────────────────────────────────
interface TaskProgress {
  current: number; total: number;
  message?: unknown; // may be string or structured object
}
interface BackgroundTask {
  id: ID; kind: TaskKind; status: TaskStatus;
  progress?: TaskProgress | null;
  result?: MarcBatchImportReport | MaintenanceResponse | null;
  error?: string | null;
  createdAt: string; startedAt?: string | null; completedAt?: string | null;
  userId: ID;
}
interface MarcBatchImportReport {
  batchId: ID; imported: string[];
  failed: Array<{ key: string; error: string; existingId?: string }>;
}
interface MaintenanceResponse {
  reports: Array<{ action: string; success: boolean; details: Record<string, number>; error?: string }>;
}

// ── Import Report ─────────────────────────────────────────────
interface ImportReport {
  action: ImportAction;
  existingId?: string;
  warnings: string[];
  message?: string;
}

// ── Item (physical copy) ──────────────────────────────────────
interface Item {
  id: ID; biblioId: ID; sourceId: ID | null; barcode: string | null;
  callNumber: string | null; volumeDesignation: string | null; place: string | null;
  borrowable: boolean; circulationStatus: string | null; notes: string | null;
  price: string | null; createdAt: string; updatedAt: string | null;
  archivedAt: string | null; sourceName: string | null;
}
```
