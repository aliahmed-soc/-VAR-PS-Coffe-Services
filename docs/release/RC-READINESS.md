# Release-candidate readiness

Status date: 2026-08-31.

Packaged HEAD: `0a85f7eace1d222c49be5db48ed4528789bbdd33`

Physical GUI UAT phases A–L all pass on this build. It is the sixth candidate: each earlier
one was superseded by a defect that physical UAT found and that is written up below.

`24c0bf349b9a2a9a437811df1f8944ec4fd006c7` was superseded: a clean install could authenticate with Supabase but could not resolve a branch because `login_online` read empty local `user_branch_roles`. First-run now downloads RLS-visible reference data in Rust, caches it transactionally, then creates the session and offline PIN.

`da192b3f89ad6c780228471383f810e821736fed` was superseded during physical GUI UAT. The
`PSC_SUPABASE_ANON_KEY` build secret held the project's legacy `service_role` JWT rather
than the publishable key. The pre-compile guard only substring-matched `service_role`,
which a base64url JWT payload never contains, so the elevated key was compiled into the
installer. The desktop runtime refused it exactly as designed (`elevated_key_forbidden`),
which surfaced as `auth: cloud not configured` on every GUI login, blocking UAT.

Remediation: the build secret now holds the publishable key; the project's legacy JWT API
keys are disabled, so the exposed `service_role` JWT is dead (`401`); the pre-compile guard
and the artifact validator now decode JWT role claims (`scripts/cloud-key-guard.mjs`,
covered by `tests/contract/cloud-key-guard.test.ts`); the two installer artifacts that
embedded the key were deleted.

Physical GUI UAT phase G found a cloud-only defect: `apply_domain_event('payment.reversed')`
returned the order to `checkout_pending` but left `receipt_number` / `receipt_snapshot` in
place, and `order.paid` refuses to overwrite a stored receipt. Every repayment after a
reversal therefore failed on the hosted RPC with `receipt_already_stored` (`400`) and retried
forever, while the till had already closed the order locally on a fresh receipt. Migration
`20260831000100_reversal_retires_receipt` makes the reversal retire the receipt and repairs
the orders the old function already stranded. The desktop binary is unaffected, so no
rebuild was required. Covered by `supabase/tests/payment/atomicity.sql` (repay after
reversal) and `tests/contract/payment-atomicity.test.ts`; `scripts/run-pg-tests.sh` now
applies every migration in order so a later redefinition cannot escape the contracts.

Physical GUI UAT then found a second blocking defect, this one in the desktop. `take_cash`
built the receipt number from the count of orders currently holding a receipt, while
`receipt_number` carries a UNIQUE index that has no branch in it. A repayment retires one
number and takes another, so the count then points back at a live number: after the phase G
repayment, every further ticket failed to close with `UNIQUE constraint failed:
orders.receipt_number`. The same counter also gave two branches the same `B-<day>-0001`,
which the cloud's global unique index would reject, so a second branch could never converge.
The counter is now the highest number already issued for that branch and day, and the branch
code is part of the number. Covered by `src-tauri/tests/receipt_numbering.rs`. The failed
payment rolled back cleanly (no payment row, no outbox event), which is itself recorded
evidence for payment atomicity.

A third defect surfaced once the receipt fix was installed. The connectivity badge read
`app_health` on mount and inside `run()` only, so nothing refreshed it while the sync worker
drained the outbox in the background: with the hosted database already holding the order and
the backend reporting `ONLINE • SYNCED`, the till went on displaying `OFFLINE • 2 UNSYNCED`
for as long as the operator did not touch it. That badge is what a cashier reads before
closing the shop, so it now polls every `HEALTH_POLL_MS`. Covered by
`tests/contract/sync-badge.test.ts`.

Phase J found the fourth and worst one. Restore is the only path that swaps the database
file underneath SQLite, and it left the live `-wal`/`-shm` behind under the old name. A
foreign WAL is only validated against its own frame checksums, so SQLite recovered pages
belonging to the displaced database into the restored one: the till came up with
`wrong # of entries in index idx_inv_movements_product` and then refused every sale with
`database disk image is malformed`. The sidecars now move with the copy they describe, which
also leaves the `.pre-restore` copy complete. Fixing that exposed the defect underneath it:
a restored backup can carry a device counter behind what the cloud already accepted, and the
cloud demands exactly `last_applied + 1`, so the till would have been stuck on `sequence_gap`
forever. Reconciliation now moves the counter past every sequence in the pulled receipts, and
never backwards. Both covered by `src-tauri/tests/restore_reconciliation.rs`. Pull-before-push
itself behaved correctly throughout: the newer cloud sale was never overwritten by the older
restored state.

Fixing those two exposed the third rewind on the same path, and the most damaging. The
receipt allocator reads the highest number this till has already issued out of its own orders
table, and a restored backup is missing orders the cloud still holds, so it re-issued
`UAT1-20260831-0001`. The cloud's global unique index refused `order.paid` with a `409` and
kept refusing it: the sale showed paid on the till, sat in `checkout_pending` in the cloud,
and because the cloud demands strict sequence order that one stuck event blocked every later
event from the same till for good. Reconciliation now records the receipts the cloud has
already issued in `receipt_high_water`, and `take_cash` allocates above both that mark and
its own orders. Covered by `src-tauri/tests/restore_reconciliation.rs`.

The last one came out of re-checking that today's report ignores an unpaid ticket. Voiding a
whole ticket only flipped the order to `void`: `void_open_order` never touched the lines, so
each stayed `active` and the units it had already deducted were never credited back. A
cashier voiding a mistyped ticket lost that stock for good, and because
`apply_domain_event('order.voided')` had the same gap, local and cloud agreed on the wrong
number, so no reconciliation could ever surface it. The ticket void now retires each line
through the same per-line path a cashier uses, which credits the stock, writes the
`sale_void` movement and carries its own event, so the cloud converges through the
already-correct `order.item_voided` handler and needs no migration. The per-line helper moved
into the caller's transaction so a ticket still voids atomically. Covered by
`src-tauri/tests/order_void_stock.rs` and `supabase/tests/inventory/order_void.sql`.

The first attempt at that fix gave `order.voided` its own stock-returning branch in the RPC.
`inventory_movements.origin_event_id` is UNIQUE, so a multi-line ticket cannot credit more
than one line from a single event; the new tests caught it in CI before it shipped.

Known non-blocking defects, deliberately not fixed during UAT:

- `upsert_product` and `upsert_payment_method` in `src-tauri/src/auth/reference.rs` omit
  `name_ar` (and `barcode`, `image_key`, `requires_reference`) from their `DO UPDATE` lists,
  so an Arabic name corrected in the cloud never reaches a till that already cached the row.
  Prices, pricing rate, stock, station names and active flags all do refresh. A fresh
  install is unaffected.
- `persist_snapshot` never prunes reference rows for branches the signed-in user is not
  assigned to, so a device where a multi-branch admin has logged in keeps the other branch's
  stations and inventory cached. Not an isolation breach: hosted RLS returns nothing
  cross-branch (`403` on cross-branch writes) and `list_stations` filters by the session
  branch. The Rust test named `cashier_bootstrap_drops_foreign_branch_rows` only proves
  those rows are never *cached*, not that they are dropped.
- `branch.sqlite.pre-restore` is never cleaned up, so `restore_reconciliation_required` is
  set again on every later start. Harmless — the tick pulls before pushing and clears it, and
  the sequence and receipt marks only ever move forward — but the badge shows
  `RECONCILE REQUIRED` for a few seconds on each launch of a till that was ever restored.
- The UAT bootstrap's Arabic product names were stored as mojibake because the PowerShell
  Management-API helper posts its body as ISO-8859-1. Hosted data has been repaired over a
  UTF-8 client. Production bootstrap must not go through that helper.
- One UAT1 walk-in ticket carries the residue of the void defect above: the order is `void`
  while its single line is still `active`, and `UAT-DRINK` at UAT1 reads 17 instead of 18.
  Only a pre-fix build could produce it, and it is disposable UAT data that
  `uat_cleanup.sql` removes, so it was left rather than repaired by migration — a repair
  would have had to fabricate the `origin_event_id` and `created_by` that
  `inventory_movements` requires.
- One UAT1 order (`9c2755c8`) sits in `checkout_pending` in the cloud with no payment and no
  receipt: the residue of the receipt-collision defect, whose `order.paid` the cloud refused.
  `payment.captured` is sequencing-only by design, so it carries no money and no receipt and
  cannot double-count. Also disposable UAT data.

Hosted migrations `20260829000100` / `20260829000200` / `20260829000300` /
`20260831000100` remain applied. The void fix needed no migration.
UAT Auth users and UAT1/UAT2 reference data remain. Disposable API-acceptance orders/receipts/sequences were reset; inventory baseline is 20.

## Green / proven

| Area | Evidence |
| --- | --- |
| contracts, cafe-domain, postgres, tauri-windows | [CI 33348312612](https://github.com/aliahmed-soc/-VAR-PS-Coffe-Services/actions/runs/33348312612) on `0a85f7e` |
| First-login bootstrap tests | `src-tauri/tests/first_login.rs`, `first_login_repro.rs` |
| Unsigned UAT NSIS | [Package Windows NSIS 33348668730](https://github.com/aliahmed-soc/-VAR-PS-Coffe-Services/actions/runs/33348668730) |
| Packaged commit | `0a85f7eace1d222c49be5db48ed4528789bbdd33` |
| Publishable key in logs | present, value not printed (`***`) |
| Installer file | `PlayStation Cafe POS_0.1.0_x64-setup.exe` (4,462,295 bytes) |
| SHA-256 | `0511b3336d710e73121b235da491eda2acfaa540ea20a3f2fdc0a3efcaee4efb` |
| Artifact name | `playstation-cafe-nsis-unsigned-0a85f7e` |
| Installer secret scan | `secret_string_hits: []` |
| Install / upgrade / uninstall smoke | passed |
| Physical GUI UAT phases A–L | passed; per-item evidence in `acceptance-checklist.md` |
| UAT1 writer `device_id` | `69c2a623-3414-4713-ba43-97d1cf0d4caf`, unchanged across four installs |
| UAT1 sequence | local next 49, cloud 1–48 contiguous, 48 receipts, no gaps or duplicates |
| UAT2 writer `device_id` | `aa4f4327-908d-449d-86fd-467a6bc22227`, cloud 1–4 |
| Signing | unsigned (expected) |

Ordinary MVP feature work is stopped. No GitHub Release.

## External / operational remaining

- Authenticode signing (nothing else blocks it)
- Replace UAT-only 3000 minor/hour rate and UAT products before live trading
- Run `supabase/bootstrap/uat_cleanup.sql` once, to drop the disposable UAT data described above
- A second physical PC for UAT2 was unavailable; UAT2 ran as an isolated second Windows
  profile on the same machine with its own app-data directory and its own registered writer
