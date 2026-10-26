# Serials management (Koha-aligned)

**Status:** proposed  
**Date:** 2026-10-26  
**Issue:** #282

## Default product decisions (to be validated by the product owner)

These are the defaults from #282. They are not confirmed.

- Item creation on receipt is a per-serial-subscription setting (`create_items`), on by default. The subscription form shows that checkbox beside the frequency, already checked, so clearing it is one click. Daily press is usually not itemised; the control must not be buried. The barcode is scanned or generated. `items.volume_designation` is the issue display label. The copy's `site_id` is the serial subscription's site.
- Latest issue: per-serial-subscription setting, default "in-library use only until the next issue arrives" (Koha `makePreviousSerialAvailable`).
- Holds on periodicals: issue-level (a hold pinned to the copy) only for the MVP. No title-level periodical hold in the MVP.
- One serial subscription per site. Adding another site means a second serial subscription on the same periodical biblio.
- Fund and annual cost are stored on the serial subscription. A purchase order on renewal comes later. The annual cost is not the copy price.
- Vendor claims: email from Elidune, plus a CSV export.
- Existing periodical copies are left untouched. The holdings statement is free text on the serial subscription.

Librarian input, also unconfirmed, and included in the data model below:

- The retention rule moves to Phase 2 (keep the last N issues, or the last N months). Discarded issues go through the existing weeding flow (#139, `items.weeding_status`) with reason `retention period ended`. Serials never deletes a copy.
- Fund and cost on a serial subscription are optional (donations, free municipal magazines, deposits from the central library). Vendor is optional for the same reason. `cost_cents` is the annual serial subscription cost in integer euro cents. It is never copied onto the item. Optional `issue_price_cents` is the per-issue price in cents. On receipt, `items.price` is set from `issue_price_cents` when that field is set. When it is null, `items.price` stays empty and lost-item billing (#261) uses the default replacement amount, not the annual cost. A patron who loses a 6 EUR magazine must not be billed 120 EUR.
- `grace_days` is the per-serial-subscription delay before an expected issue becomes `late`. The scheduler marks `late` when the calendar date is after `planned_date + grace_days`. Null uses the frequency default (daily 2, weekly 3, twice a month or every two weeks 7, monthly 15, every two months or quarterly 30, twice a year or yearly 45). A stored value, including 0, overrides that default.
- Every issue carries a human-readable display label that combines numbering and date (for example `n° 412, mars 2026`). The prediction engine generates it. It is stored on the issue row. Desk staff, the OPAC, issue-level holds, and vendor claims all refer to that label.
- Frequency labels in the UI are translations. `bimensuel` is twice a month (`2/month`). Once every two weeks is `tous les quinze jours` (quinzomadaire), not `bimensuel`. The other usual names are `quotidien`, `hebdomadaire`, `mensuel`, `bimestriel`, `trimestriel`, `semestriel`, and `annuel`. The English seed `description` stays as stored. MVP 4 looks up `serials.frequencies.*` from `unit`, `issues_per_unit`, and `units_per_issue`, and falls back to `description` when no key exists. There is no frequency screen in this change.

Naming: **serial subscription** in new docs, API fields, and code. The word `subscription` alone remains patron membership (`subscription_expired`, and so on).

## Goal

Full serials (periodicals) management, using Koha's Serials module as the functional reference, compatible with the current `dev` schema. Work stays on `dev`. New migrations start at `004`. Migrations `001`, `002`, and `003` are never edited.

This document covers the whole feature. The first implementation (this change) is Phase 0 plus MVP 1: the spec, `serials_rights`, frequencies, numbering patterns, and the prediction engine.

## Architecture

1. **Prediction is a pure Rust module** (`server/src/serials/prediction.rs`). It does not touch the database. Inputs are the frequency, the numbering pattern, the anchor date (`first_issue_date`), the absolute `start_slot`, the starting counter values (and the inner-loop cursors when continuing a horizon), the length, absolute irregularity slot numbers, and the yearly skip rules. The output is the expected issue list (planned date, counter values, display label) and `nextState`, including `next_slot`. Unit tests cover irregular issues, year and month-end rollover, counter resets, renumbering, two horizons matching one horizon, and yearly skips.
2. **Expected issues are persisted rows** (`serial_issues`), written progressively (a horizon, then "generate next"), not computed on the fly. Receiving, late detection, and claims update those rows. Re-running prediction does not rewrite rows that already exist. The MVP 1 `POST /serials/predictions` endpoint is a preview only: it calls the engine and returns the list. It does not insert rows. `serial_issues` is created in the migration of the phase that first writes it (MVP 2 / MVP 3).
3. **Receiving an issue (MVP 3) reuses the acquisitions item path.** `AcquisitionsService::receive_order` builds an `Item` and calls `CatalogService::create_item`. A missing barcode uses `allocate_local_item_barcode`. Serials receipt calls those same functions. It does not grow a second `INSERT` into `items`. Shared field defaults (weeding `on_shelf`, the availability pair) may move into one constructor used by both call sites so the two paths do not drift. The insert stays `CatalogService::create_item`.

## What this reuses

| Existing piece | Use in serials |
|---|---|
| Biblio `MediaType::Periodic` | The periodical record a serial subscription hangs on. No new media type. |
| `vendors` | Optional supplier on the serial subscription, and the addressee of claims. |
| `acquisition_funds` | Optional fund on the serial subscription. Renewal may later add a purchase-order line (Phase 4). |
| `CatalogService::create_item` and `allocate_local_item_barcode` | The only way a received issue becomes a copy. |
| `items.volume_designation` | Set to the issue display label at receipt. |
| `items.site_id` | The serial subscription's site. |
| `items.weeding_status` and the weeding flow (#139) | Retention discards. Reason text `retention period ended`. |
| Copy-level holds | Issue-level holds are ordinary holds on the received copy. |
| `email_outbox` and `email_templates` | Vendor claim emails (Phase 2) and patron new-issue alerts (Phase 3). |
| `services/scheduler.rs` | Phase 2 late-issue job, same process as the overdue-reminder task. |

## Rights

New column `account_types.serials_rights` `varchar(1)`, letters `n` / `r` / `w`, same ordering as `items_rights` and `acquisitions_rights`: none < read < write. `o` (own) is not a serials level.

It is distinct from `items_rights` (cataloging) and from `acquisitions_rights` (vendors, funds, orders). A cataloger without `serials_rights` cannot predict or edit patterns. A serials librarian without `items_rights` write still receives copies through the serials receipt path in MVP 3, which is why receipt is authorized by `serials_rights` write, not by cataloging write. The item insert itself stays the shared catalog function.

JWT claim `serialsRights` on `UserRights`, with `#[serde(default)]` so tokens issued before the column existed deserialize as none. Helpers: `require_read_serials`, `require_write_serials`.

Seed: `librarian` and `admin` are `w`. `guest`, `reader`, and `group` stay `n` (column default). Custom types omit the field and are stored as `n`.

The settings UI lists the column next to acquisitions so a save does not drop it. There is no serials screen in MVP 1.

`serials_rights` does not grant the internal catalog view.

Routes are mounted with the other staff APIs. There is no feature flag in MVP 1.

## Reference data (MVP 1)

### `serial_frequencies`

Koha's frequency fields.

| Column | Meaning |
|---|---|
| `id` | `bigint` primary key. Seed rows use stable ids 1–14. New rows use a snowflake. |
| `description` | Label, for example `1/month`. Not unique. |
| `unit` | `day`, `week`, `month`, `year`, or NULL. NULL is the irregular frequency: staff will type dates later; the engine cannot invent them. |
| `issues_per_unit` | How many issues fall in one cycle. At least 1, at most 366. |
| `units_per_issue` | How many units one cycle lasts. At least 1, at most 1000. |
| `display_order` | Sort key. |

Seeded catalog (Koha's usual English sample: `issues_per_unit` / `units_per_issue`):

| Id | Description | Unit | Issues per unit | Units per issue |
|---|---|---|---|---|
| 1 | 2/day | day | 2 | 1 |
| 2 | 1/day | day | 1 | 1 |
| 3 | 3/week | week | 3 | 1 |
| 4 | 1/week | week | 1 | 1 |
| 5 | 1/2 weeks | week | 1 | 2 |
| 6 | 1/3 weeks | week | 1 | 3 |
| 7 | 1/month | month | 1 | 1 |
| 8 | 1/2 months | month | 1 | 2 |
| 9 | 1/3 months | month | 1 | 3 |
| 10 | 2/month | month | 2 | 1 |
| 11 | 1/6 months | month | 1 | 6 |
| 12 | 1/year | year | 1 | 1 |
| 13 | 1/2 years | year | 1 | 2 |
| 14 | Irregular | NULL | 1 | 1 |

Seed rows are ordinary data. Staff may edit or delete them. Once a serial subscription references a row, the foreign key blocks delete (`409`).

The UI does not display `description` as the patron-facing or desk label. MVP 4 translates from the structural key (`issues_per_unit`, `units_per_issue`, `unit`):

| Key | Matches | French |
|---|---|---|
| `serials.frequencies.twiceDaily` | 2/day | deux fois par jour |
| `serials.frequencies.daily` | 1/day | quotidien |
| `serials.frequencies.threePerWeek` | 3/week | trois fois par semaine |
| `serials.frequencies.weekly` | 1/week | hebdomadaire |
| `serials.frequencies.fortnightly` | 1/2 weeks | tous les quinze jours |
| `serials.frequencies.everyThreeWeeks` | 1/3 weeks | toutes les trois semaines |
| `serials.frequencies.monthly` | 1/month | mensuel |
| `serials.frequencies.everyTwoMonths` | 1/2 months | bimestriel |
| `serials.frequencies.quarterly` | 1/3 months | trimestriel |
| `serials.frequencies.twiceMonthly` | 2/month | bimensuel |
| `serials.frequencies.semiannual` | 1/6 months | semestriel |
| `serials.frequencies.annual` | 1/year | annuel |
| `serials.frequencies.everyTwoYears` | 1/2 years | tous les deux ans |
| `serials.frequencies.irregular` | NULL unit | irrégulier |

`ui/src/utils/serialFrequencyLabel.ts` maps a frequency to that key. A row staff created with some other cycle has no key, so the screen shows `description`. English, German, and Spanish strings live beside the French ones. The seed text is not rewritten.

### `serial_numbering_patterns`

Koha-style formula. Three counters, X, Y, and Z.

| Column | Meaning |
|---|---|
| `label`, `description`, `display_order` | Catalog metadata. |
| `formula` | Numbering method, for example `n° {X}, {MonthName} {Year}`. |
| `locale` | `fr` (default) or `en`. Month and day names in the label use this locale. French names are lowercase (`mars`). English names are capitalized (`March`). |
| For each of X, Y, Z | `name` (optional staff caption), `add`, `every`, `when_more_than`, `set_back_to`, `format`, `zero_pad`. |

`format` is `number` (optional zero-pad, width 1–12), `day_name` (counter 1 = Monday … 7 = Sunday), `month_name` (counter 1–12), or `season` (1 winter/hiver, 2 spring/printemps, 3 summer/été, 4 autumn/automne). A counter value outside the name list is rendered as the integer.

`when_more_than` and `set_back_to` are both set or both null. `every` is at least 1.

Migration `004` seeds three patterns (stable ids 1–3, same idea as the frequency ids) so a serial subscription can be created before anyone writes a pattern: `n° {X}`, `vol. {X}, n° {Y}` (volume every 12 issues, issue number back to 1), and `n° {X}, {MonthName} {Year}`. Staff may edit or delete them. A referenced row cannot be deleted (`409`).

### Prediction rules

The engine emits the **current** counter values, then advances. Starting values are the first issue of this call. Skipped slots do not advance counters.

`first_issue_date` is the anchor and does not move between horizons. Dates are always computed from that anchor and the absolute slot, so a monthly that starts on the 31st still lands on the 31st after February, and `2/month` keeps its place inside the cycle. `start_slot` is the absolute 1-based slot this call emits first (1 for a new serial subscription). `nextState.next_slot` is the absolute slot the following horizon must pass back as `start_slot`, together with the counter values and inner loops, and the same anchor. Two horizons of 6 issues are defined to equal one horizon of 12. The same holds for 6 then 6 months against 12 months, and for weeks.

Advance, per counter, after an emitted issue:

1. Inner loop starts at 0 for a new serial subscription and is stored between horizons.
2. Inner loop increments by 1.
3. When the inner loop reaches `every`, add `add` to the value and set the inner loop back to 0. `add` may be negative (a countdown). Addition saturates at `i32` bounds.
4. When this step **crosses** `when_more_than` (the value before the add was less than or equal to the limit, and the value after the add is strictly greater), set it to `set_back_to`. The limit is not applied again to the reset value. A `set_back_to` at or below the limit starts a new cycle (12 rolls to 1, then 2). A `set_back_to` above the limit keeps counting (3 rolls to 100, then 101).

Volume example: X adds 1 every issue and resets when more than 12, back to 1. Y adds 1 every 12 issues. Issues 1–12 stay on volume 1; issue 13 is number 1 of volume 2. After that issue the Y inner loop is 1, because the volume advanced on the step after issue 12 and issue 13 counts as the first step of the next volume.

Date variables in the formula, taken from the planned date (not from the counters): `{Year}`, `{Month}`, `{Day}` (unpadded numbers), `{MonthName}`, `{DayName}`. `{MonthName}` is replaced before `{Month}`, and `{DayName}` before `{Day}`. Unknown `{tokens}` are left as written.

**Dates.** For one issue per cycle, the step is Koha's: 1/week is 7 days, 1/2 weeks is 14 days, 1/month is one month, 1/year is one year, 1/3 months is three months. Months and years are added from the **first** issue date, so 31 January stays the 31st when the target month has 31 days and clamps when it does not (31 January → 28 February → 31 March). February 29 plus one year is February 28. The series does not drift by chaining from the clamped date.

When `issues_per_unit` is greater than 1, issues are spaced evenly inside the cycle, measured from the cycle start:

- Days and weeks: offset = `slot * cycle_days / issues_per_unit` (integer division). `2/day` publishes both issues on the same date, then moves one day. `3/week` from a Monday is Monday, Wednesday, Friday.
- Months and years: the cycle's day span is split the same way. `2/month` from 1 January is the 1st and the 16th in a 31-day month.

This even split is the Elidune rule for several issues per unit. One-issue frequencies match Koha. The product owner can revisit the intra-cycle spacing.

**Irregularities** are absolute 1-based publication slots, counted from the anchor, that are not issued (a weekly that skips a week). They are not renumbered per horizon: slot 8 is still slot 8 on the second call. The date advances as if the slot existed. No row is emitted and counters do not move. `0` is rejected. Duplicates are ignored. This is not the irregular frequency (NULL unit). A NULL unit cannot predict dates; the preview returns `422`. Staff enter those dates by hand in MVP 3.

**Yearly skips** avoid re-entering those slot numbers every year. They live on the serial subscription (the preview accepts them inline) and the engine applies them on top of the absolute irregularities. A skip matches the **planned date** (month, or month and day), never a slot number. That still holds when the first issue is not in January, and when the skipped month falls in a later horizon.

- `recurringSkips`: `{ "month": 8 }` skips every planned date in August (11 issues in a calendar year for a monthly). `{ "month": 7 }` and `{ "month": 8 }` together are 10 issues. `{ "month": 12, "day": 25 }` skips only 25 December; a weekly that does not fall on that day is unchanged.
- `combinedIssues`: `{ "months": [7, 8], "monthName": "juillet-août" }`. The first month is published. Later months are skipped. `{MonthName}` on the published issue is the given text, so the July row reads `juillet-août` and August has no row. The same rule applies again the next year. A full-month recurring skip of the published month is rejected. The first month of one combined issue cannot also be a later month of another.

A skipped month still consumes a frequency slot, so the following issue keeps its cycle position.

**Length** is one of:

- `issues`: that many emitted issues (irregularities extend the calendar).
- `weeks` or `months`: the window opens on the planned date of `start_slot`, which is the anchor only when `start_slot` is 1. Emit while `planned_date` is strictly before that date plus the length. A weekly for 2 weeks from slot 1 yields two issues, not three. A renewal of 12 more months starting at slot 13 covers the next 12 months; it does not stop on the first year's end. Two calls of 6 months produce the same issues as one call of 12 months, including a monthly that starts on the 31st and an August skip (also when the first issue is in October, so August is not slot 8). Two calls of 6 weeks produce the same issues as one call of 12 weeks for a weekly. A window that emits nothing is `422`, not an empty list.

The preview rejects a length under 1, an issue count over 500, or a date window that would emit more than 500 issues. Walking more than 5000 slots in one horizon (a long skip list) is the same error. The 5000 is counted from `start_slot`, not from slot 1, so a daily can still predict a horizon after slot 5000.

The response includes `nextState` (values, inner loops, and `nextSlot` after the last emitted issue). MVP 2 stores that cursor and the original anchor on the serial subscription. The next horizon calls the engine with the same `firstIssueDate`, `startSlot` = `nextSlot`, and the stored counters. The preview accepts optional `inner`, `startSlot` (default 1), `recurringSkips`, and `combinedIssues` so a horizon can be tried before the table exists.

## API (MVP 1)

All routes are under `/api/v1`. OpenAPI tag `serials`.

| Endpoint | Right |
|---|---|
| `GET /serials/frequencies` | read |
| `GET /serials/frequencies/{id}` | read |
| `POST /serials/frequencies` | write |
| `PUT /serials/frequencies/{id}` | write |
| `DELETE /serials/frequencies/{id}` | write |
| `GET /serials/numbering-patterns` | read |
| `GET /serials/numbering-patterns/{id}` | read |
| `POST /serials/numbering-patterns` | write |
| `PUT /serials/numbering-patterns/{id}` | write |
| `DELETE /serials/numbering-patterns/{id}` | write |
| `POST /serials/predictions` | read |

`POST /serials/predictions` takes either `frequencyId` or an inline `frequency`, and either `patternId` or an inline `pattern`, plus `firstIssueDate` (the anchor), `starting` (`x`, `y`, `z`), optional `inner`, optional `startSlot` (default 1), `length` (`{ "kind": "issues"|"weeks"|"months", "count" }`), optional absolute `irregularities`, optional `recurringSkips`, and optional `combinedIssues`. It returns `issues[]` (`sequence`, `slot`, `plannedDate`, `displayLabel`, `x`, `y`, `z`) and `nextState` (`x`, `y`, `z`, `innerX`, `innerY`, `innerZ`, `nextSlot`).

List endpoints take `q`, `page`, and `perPage`. Ids in JSON are strings. Validation failures are `400`. An irregular frequency, a preview over the 500-issue cap, or a weeks/months horizon that emits nothing is `422`. Unknown ids are `404`. Missing token is `401`. The wrong right is `403`.

Mutations are audited (`serial_frequency.*`, `serial_numbering_pattern.*`). The preview is not.

## Later data model

Migration `004` does not create these tables. Migration `005` creates `serial_subscriptions` and `serial_issues`.

### `serial_subscriptions` (MVP 2)

One open serial subscription per (`biblio_id`, `site_id`).

- `biblio_id` not null, the periodical record.
- `site_id` not null.
- `vendor_id` null → `vendors`. `fund_id` null → `acquisition_funds`. `cost_cents` bigint null. This is the annual serial subscription cost in integer euro cents. Receipt does not copy it to the item.
- `issue_price_cents` integer null. Optional price of one issue, in cents. Receipt sets `items.price` from it. Null leaves `items.price` empty, so lost-item billing uses the default replacement amount.
- `frequency_id`, `numbering_pattern_id` not null.
- `first_issue_date` date not null. Anchor for every horizon. Not replaced by the last predicted date.
- Starting and cursor values (`value_x/y/z`, `inner_x/y/z`) and `next_slot` integer not null default 1. The next horizon passes this slot as `startSlot` with the same anchor.
- `length_kind`, `length_count`, `irregularities` (integer array of absolute slots), optional `end_date`.
- `recurring_skips` jsonb not null default `[]`. `combined_issues` jsonb not null default `[]`. Shape as in the prediction rules.
- `grace_days` integer null. Days after `planned_date` before the late job may set `late`. Null uses `default_grace_days` (`server/src/serials/grace.rs`). A stored number, including 0, wins. Irregular frequency with null grace is not auto-marked late.

Approximate interval: `units_per_issue` times 1, 7, 30, or 365 (day, week, month, year), divided by `issues_per_unit`. The default grace is 1 under 1 day, 2 under 4 days, 3 under 10 days, 7 under 20 days, 15 under 45 days, 30 under 120 days, and 45 after that.

| Cycle | Grace |
|---|---|
| 2/day | 1 |
| 1/day, 3/week | 2 |
| 1/week | 3 |
| 1/2 weeks, 2/month | 7 |
| 1/3 weeks, 1/month | 15 |
| 1/2 months, 1/3 months | 30 |
| 1/6 months, 1/year, 1/2 years | 45 |
- `status`: `active`, `expired`, `closed`. `active` is the open subscription. Only one `active` row per (`biblio_id`, `site_id`).
- `create_items` boolean not null default true. The subscription form shows it as a checkbox beside the frequency, checked by default. One click clears it. Daily press is usually not itemised.
- `latest_issue_library_use_only` boolean not null default true.
- `holdings_statement` text null. Free text. Existing copies are not rewritten.
- Optional `start_date`. Prediction does not use it. `first_issue_date` stays the anchor. Optional `end_date` is inclusive: a planned date after it is not generated.
- `call_number` (varchar 100), `location` (varchar 200), `public_note`, `internal_note`, and `notes`. Receipt (MVP 3) copies the call number, location, and notes onto the item. They are not prediction inputs.
- timestamps.

Creating a serial subscription predicts the first horizon of `serial_issues` (status `expected`) and stores `nextState` in the cursor columns. Horizon: issues whose planned date falls in the next 12 months, and not past the serial subscription length or `end_date`, capped at 500. `POST /serials/subscriptions/{id}/generate` appends the next horizon from `next_slot` and the same anchor. It does not recompute existing rows. Changing prediction inputs after issues exist is refused. Closing sets `status` to `closed` and keeps the issue rows. Search is `GET /serials/subscriptions` with `q` (title or an ISSN on a linked series or collection; `biblios.issn` is still out of scope), `siteId`, `vendorId`, and `status`.

### `serial_issues` (MVP 2 schema, written by prediction; MVP 3 fills receipt)

- `serial_subscription_id` not null.
- `planned_date` date not null.
- `display_label` text not null. This is what the desk, the OPAC, holds, and claims show.
- `sequence` integer null. `slot` integer null. `slot` counts skipped irregularities for a regular issue. `sequence` counts emitted regular rows. A supplement (`supplement` true) may omit both. A regular row must have both: `CHECK (supplement OR slot IS NOT NULL)` and `CHECK (supplement OR sequence IS NOT NULL)`. Uniqueness is a partial index on (`serial_subscription_id`, `slot`) where `slot` is not null, and a partial index on (`serial_subscription_id`, `sequence`) where `sequence` is not null, so several hors-série rows do not collide with each other or with predicted issues. Generation ignores supplement rows: the next horizon continues from `next_slot`, and its sequences continue from the count of non-supplement rows.
- `value_x`, `value_y`, `value_z`.
- `status` (below).
- `supplement` boolean not null default false. Staff can insert a supplement without rerunning prediction (MVP 3).
- `item_id` null until receipt.
- `received_at`, `notes`, timestamps.

### Statuses

Stored as text, not Koha's integers.

| Status | Meaning |
|---|---|
| `expected` | Predicted, not received. |
| `arrived` | Received. |
| `late` | Still expected after `planned_date + grace_days`. Set by the scheduler, not by recomputing dates. |
| `missing` | Staff marked it missing. |
| `claimed` | A vendor claim has been sent for this row. |
| `not_available` | Will not arrive and should not be claimed (Koha "not available"). |
| `deleted` | Soft-deleted. The row stays so claims and history keep a target. |
| `bound` | Phase 4, gathered into a bound volume. |

### Claims, routing, alerts (later tables)

- `serial_claims`: `serial_issue_id`, vendor, sent timestamp, method `email` or `csv`, optional `email_outbox` id. The email body names the display label. CSV export is the other path.
- `serial_routing_lists` and `serial_routing_list_entries`: ordered patron ids on a serial subscription (Phase 3).
- `serial_issue_alerts`: patron opt-in, unique (`user_id`, `serial_subscription_id`). A new `arrived` issue enqueues mail through `email_outbox` and an `email_templates` row (Phase 3).

### Receipt (MVP 3)

`POST /serials/subscriptions/{id}/issues/{issueId}/receive` moves an `expected` or `late` issue to `arrived` and sets `received_at`. It does not change any other issue's status. The handler locks the subscription row (`SELECT … FROM serial_subscriptions WHERE id = $1 FOR UPDATE`), then that issue row, then inserts the copy and marks the issue arrived in the same transaction. Receipts of the same subscription therefore run one at a time, so only the newest arrived regular issue stays non-borrowable. A second receipt of the same issue that loses the race returns 409 and leaves no copy. Status changes that can move which issue is latest take that same subscription lock before any issue row. An expected issue becomes `late` only through `POST /serials/subscriptions/{id}/mark-late`, when the library's local date (`library_info.timezone`, via `local_date`) is after that issue's own `planned_date` plus the effective grace (`grace_days` when stored, including 0, otherwise the frequency default). An irregular subscription with no grace is not auto-marked. The scheduler that calls this remains Phase 2.

`POST /serials/subscriptions/{id}/issues/{issueId}/status` moves an unreceived issue to `late`, `missing` (claim-ready; no claim is sent), `claimed` (from expected, late, or missing; still no email), `notAvailable` (will not be published), or `deleted`. `arrived` is only the receive route.

`POST /serials/subscriptions/{id}/supplements` inserts a hors-série with `supplement` true and null `slot` and `sequence`. Prediction is unchanged.

For each issue moved to `arrived`, when `create_items` is true:

- Barcode from the scan, otherwise `allocate_local_item_barcode`.
- The copy is `Item::received_copy`, the same constructor acquisitions receipt uses. The insert is the statement `CatalogService::create_item` uses (`insert_item_row`), run inside the receipt transaction after the subscription row and the issue row are locked. The search index is updated after that transaction commits.
- `volume_designation` = `display_label`.
- `site_id` = the serial subscription site.
- `call_number`, `public_note`, and `internal_note` are copied. `notes` is copied into the item internal note only when `internal_note` is empty. `location` stays on the subscription: `items.place` is a numeric shelf code, not free text.
- `items.price` = `issue_price_cents` when that column is set. When it is null, the item price stays empty. `cost_cents` is not the item price.
- `borrowable` follows `latest_issue_library_use_only`. The latest issue is the arrived regular issue with the most recent `planned_date` (then the highest slot), not the most recently received one. Supplements never count. The rule touches two copies only: the new latest copy becomes non-borrowable, and the previous latest copy becomes borrowable again only when this rule had made it non-borrowable. `serial_issues.library_only_borrowable_before` stores that previous `items.borrowable` while the rule holds the copy. A librarian's own non-borrowable choice, including one made before the issue became the latest, is left as it is. Receiving n° 413 does not mark n° 412 arrived; if 412 arrives later it is loanable immediately and 413 stays in the library.

No migration `006`: receipt uses the columns from `005`.

Irregular issues (extra skips) and supplements are staff edits on rows, not a second prediction model.

### Retention (Phase 2)

Per serial subscription: keep the last N arrived issues, or the last N months. Issues past the rule are handed to the weeding flow with reason `retention period ended`. The copy moves through `items.weeding_status`. The serial issue row stays (status unchanged except where weeding already hides the copy). No serials delete of items.

### Circulation (Phase 2)

The in-library-only copy is the arrived regular issue with the most recent planned date, if the setting is on. An older issue that arrives later is lendable. Holds stay copy-level on the received item. No new hold type.

### Renewal and purchase orders (Phase 2 close/reopen, Phase 4 order line)

Close and reopen change `serial_subscriptions.status`. Renewal rolls the cursor forward and generates the next horizon. Phase 4 may add a purchase-order line on the optional fund, using the existing acquisitions order APIs. MVP 2 does not create orders.

### OPAC and ISSN

Phase 3 shows the latest arrived issues and the free-text holdings statement. It does not invent a holdings summary from old copies.

`biblios.issn` and MARC 022 are a separate slice (the last line of #282), not part of MVP 1–4.

## Phases

- **Phase 0.** This spec.
- **MVP 1 (this change).** Migration `004`: `serials_rights` (`NOT NULL DEFAULT 'n'`, like `weeding_rights`), `serial_frequencies` with the seed, `serial_numbering_patterns` with three seeded patterns. Pure prediction engine and its unit tests. Read/CRUD for frequencies and patterns, `POST /serials/predictions`, OpenAPI, rights, integration tests. No serial subscription rows. Frequency translation keys are in the UI locales; no frequency screen yet.
- **MVP 2.** Migration `005`: `serial_subscriptions` per site, optional vendor, fund, annual `cost_cents`, and `issue_price_cents`, `grace_days`, anchor date, and `next_slot`. Horizon generation into `serial_issues` continues from that cursor. A supplement may omit `slot`. No subscription UI (MVP 4).
- **MVP 3.** Receive expected or late issues onto persisted rows, create items through `Item::received_copy` and `CatalogService::create_item`, staff statuses (late, missing, claimed, not published, deleted), grace-based late detection, and supplements with a null slot and sequence. The latest in-library copy is the arrived regular issue with the newest planned date.
- **MVP 4.** UI: serial subscription form with the prediction preview, the `create_items` checkbox beside the frequency, serial collection, receive screen, search. Frequency labels use `serials.frequencies.*`. Account-type settings already expose `serialsRights` from MVP 1.
- **Phase 2.** Late-issue scheduler job and vendor claims (email + CSV). Expiration, renewal, close/reopen. Circulation: previous issue lendable, issue-level holds. Retention rule via weeding.
- **Phase 3.** OPAC latest issues and holdings text. Patron new-issue alerts. Routing lists.
- **Phase 4.** Renewal creates a purchase-order line. Binding status. Stats.
- **Separate.** Record-level ISSN (`biblios.issn` + MARC 022).

## Out of scope for MVP 1

- Serial subscription and issue tables, receipt, UI screens, claims, routing, alerts, the scheduler job, ISSN.
- Editing migrations `001`–`003`.
- A feature flag.

## Verification

- `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --lib`.
- DB integration tests: seeded frequencies, CRUD, the preview (including the `n° 412, mars 2026` label), `401` / `403` / `422`, and `serials_rights` on `librarian` / `admin` (`w`) and `reader` (`n`).
- Prediction unit tests: irregular slots, year and month-end rollover, leap day, several issues per unit, counter reset, renumbering, inner-loop continuation, the 500-issue cap, two horizons of 6 matching one horizon of 12 for issue counts and for months (31st of the month, 2 per month, an absolute irregularity, a yearly August skip, including a subscription that starts in October) and for weeks on a weekly, a 12-month renewal from slot 13, an empty horizon refused with an error, 10 and 11 issues per year, and a combined July–August issue. Grace defaults are unit-tested in `server/src/serials/grace.rs`.
