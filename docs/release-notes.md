# Release notes

## Unpaid-fine blocking uses an explicit mode

Checkout, renewal, and the card-scan `unpaid_fines` reason block on unpaid fines only when an effective amount is configured and the patron's unpaid balance is **strictly above** it.

`unpaidThresholdMode` replaces a nullable amount:

| Level | Modes | Amount |
|-------|--------|--------|
| Global circulation setting | `none` or `amount` | `unpaidThresholdCents`, required with `amount` |
| Public type | `inherit`, `none`, or `amount` | `unpaidThresholdCents`, required with `amount` |

- `amount` with `0` blocks on any unpaid fine (any balance greater than zero).
- Global `none` does not block on amount. Overdue loans and a manually blocked account still apply.
- Public type `none` never blocks on amount, even when the global mode is `amount`.
- Public type `inherit` uses the global mode.

Effective rule: type `amount` uses that type's cents; type `none` does not block; type `inherit` uses the global mode (`none` does not block, `amount` uses the global cents).

Migration `051_unpaid_threshold_nullable` adds the mode columns and backfills existing rows.

New installs start with global mode `none`. New public types start with `inherit`. An existing global `0` becomes `none` only when the library never successfully saved the fine policy (no successful `fine.policy_updated` audit). Any other stored global value, including an explicit `0`, stays mode `amount` with that value. Existing public-type `NULL` becomes `inherit`. A stored public-type value, including `0`, becomes `amount`.

Omit `unpaidThresholdMode` on PUT to leave the stored mode and cents unchanged. JSON `null` is rejected. Mode `amount` without `unpaidThresholdCents` is **422**.

To turn global amount-based blocking off, Settings → unpaid fine threshold, choose “do not block”, or `PUT /api/v1/fines/policy` with `{ "unpaidThresholdMode": "none" }`.

For one audience, set `unpaidThresholdMode` to `none` so it never blocks on amount, or to `inherit` so it follows the global setting (`PUT /api/v1/public-types/{id}`).

To block any unpaid fine, use `{ "unpaidThresholdMode": "amount", "unpaidThresholdCents": 0 }`.
