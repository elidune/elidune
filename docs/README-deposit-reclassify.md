# Reclassify historical deposit copies

Staff command for copies that were imported from MARC while the lender was stored in `items.source_id`, as if that lender were a site. The command attaches those copies to an incoming deposit batch and points `items.source_id` at the physical site that holds them.

Run it with the server binary and the same configuration the server uses (`--config` / `-c`, or `ELIDUNE_*` environment variables, including `ELIDUNE_DATABASE__URL`):

```bash
elidune-server reclassify-deposits --help
```

The database must already contain deposit batches (migration 065). The command does not apply migrations.

## Dry run, then apply

Dry run is the default and writes nothing. **Run that dry run against a copy of the production database before `--apply` on production.**

```bash
elidune-server reclassify-deposits \
  --config config/default.toml \
  --lender 12 \
  --batch-reference "CITYLIB-2024-SPRING" \
  --received-on 2024-03-01 \
  --return-due-on 2024-09-01 \
  --notes "Historical MARC import" \
  --site 3 \
  --from-source 41

elidune-server reclassify-deposits \
  --config config/default.toml \
  --lender 12 \
  --batch-reference "CITYLIB-2024-SPRING" \
  --received-on 2024-03-01 \
  --return-due-on 2024-09-01 \
  --notes "Historical MARC import" \
  --site 3 \
  --from-source 41 \
  --apply
```

`--from-source` selects every copy at that source whose `archived_at` is null. `--barcodes path` selects an explicit list instead (one barcode per line, or a CSV with a `barcode` column). Pass exactly one of those two options.

`--return-due-on` is required only when no batch with that reference exists for the lender. `--received-on` maps to `arrived_on` and defaults to today (UTC) when the batch is created. When the reference already exists and the batch is `active`, it is reused and those fields are left unchanged. A batch in any other status is refused and nothing is written.

`--format json` prints the same report as camelCase JSON. Ids in that JSON are decimal strings.

## Report

Each selected copy is one line: barcode, item id, bibliographic record id, title, old source, new site, target batch, and whether the copy is on loan or has an active hold (`pending`, `to_pick`, or `ready`).

Summary counts:

| Count | Meaning |
| --- | --- |
| to reclassify | Copies that will be updated |
| on loan | Those copies with an open loan |
| with holds | Those copies with an active hold |
| refused | Copies listed below and not written |
| already reclassified | Copies already on the target batch and the target site |

Refusals (reason code, never written):

| Code | Meaning |
| --- | --- |
| `barcode_unknown` | Barcode is not on file |
| `item_archived` | `archived_at` is set, or weeding status is `withdrawn` |
| `item_in_other_batch` | `deposit_batch_id` points at a different batch |
| `item_in_transit` | An open transit exists (`requested` or `in_transit`) |

A withdrawn copy is refused even when `--from-source` selects it. A copy with `archived_at` set is not part of a `--from-source` selection; pass its barcode to see `item_archived`. A weeding status of `candidate` is still reclassified. A transit that is `received` or `cancelled` is not open.

`already_reclassified` is a skip. Running the command again does not write a second audit row for that copy. An open transit can be reclassified later by running the command again after the transit is received or cancelled.

Records of selected copies whose normalized ISBN matches another record **outside** the selection are listed as `possible_duplicate_record` with both record ids. The command never merges or edits bibliographic records. Review those pairs and merge them in the catalog if they are the same title.

Refused copies do not abort the run. The process exits 0 when the report was produced. It exits non-zero for invalid arguments (unknown or archived lender, unknown or archived site, unknown `--from-source`, batch not `active`, missing `--return-due-on` when the batch must be created) and for database errors.

## What `--apply` guarantees

Create-or-reuse of the batch, the item updates, and the audit rows commit in **one transaction**. An unexpected error rolls the whole transaction back.

For each copy that is reclassified, the only item columns written are `deposit_batch_id`, `source_id`, and `updated_at`. Loans, holds, transits, due dates, and fines are not modified. An active loan keeps its due date; the deposit return cap is applied the next time that loan is renewed.

Each updated copy gets one audit event `deposit_batch_reclassified` (`entity_type` `item`, `user_id` null). The payload `actorType` is `script` (`actorName` `reclassify-deposits`) and includes `lenderId`, `batchId`, `oldSourceId`, and `newSourceId`.
