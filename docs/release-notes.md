# Release notes

## Unpaid-fine threshold defaults to off

Checkout, renewal, and the card-scan `unpaid_fines` reason block on unpaid fines only when an amount threshold is configured and the patron's unpaid balance is **strictly above** it.

- `0` blocks on any unpaid fine (any balance greater than zero).
- `null` disables amount-based blocking. Overdue loans and a manually blocked account still apply.

The effective threshold is the public type's `unpaidFineThreshold` when that value is set, otherwise the global `circulation_settings` threshold. If both are null, unpaid fines do not block.

New installs and newly created public types start with `null`. Existing public-type values are left as stored. An existing global `0` is changed to `null` only when the library never successfully saved the fine policy (no `fine.policy_updated` audit). An explicit `0` that was saved stays `0` and still blocks.

To disable amount-based blocking:

1. Settings → unpaid fine threshold, clear the amount and save, or `PUT /api/v1/fines/policy` with `{ "unpaidFineThreshold": null }`.
2. For one audience, clear that public type's unpaid-fine override (or send `"unpaidFineThreshold": null` on `PUT /api/v1/public-types/{id}`) so it inherits the global value.

To block any unpaid fine, set the threshold to `0` (`{ "unpaidFineThreshold": "0.00" }`).
