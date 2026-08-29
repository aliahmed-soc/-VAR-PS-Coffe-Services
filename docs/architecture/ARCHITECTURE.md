# PlayStation Café POS — canonical architecture

Frozen. Do not reopen these decisions without an explicit product change.

Priority order for remaining implementation choices:

**reliability → data integrity → simplicity → security → maintainability → offline resilience → testability → low operational complexity.**

## Product

Windows desktop POS and station timer for two café branches. One transaction-writing cashier PC per branch. Local work must succeed without internet. Cloud is the synchronized authority, not the live cashier store.

## Stack

| Layer | Choice |
| --- | --- |
| Shell | Tauri 2 |
| UI | React + TypeScript + Tailwind CSS + i18n (en/ar, RTL) |
| Domain / I/O | Rust |
| Local DB | SQLx + SQLite (WAL, foreign keys) |
| Cloud | One Supabase Pro project: Postgres, Auth, RLS, RPC |
| License | MIT |

**Forbidden:** custom API server, Prisma, Drizzle, `supabase-js` (or any Supabase client) in the webview, UI-side money math, `f64`/`f32` money.

React invokes Tauri commands only. Rust owns SQLite, pricing, payments, outbox, tokens, and every HTTP call to Supabase.

## Local-first sync

1. A cashier action writes SQLite and the transactional outbox in the same transaction.
2. Each event has a UUID `event_id` and a strict per-device `local_sequence`.
3. A Tokio worker (independent of React) pushes via `apply_domain_event`.
4. Server accepts `local_sequence == last + 1`. Duplicate `event_id` → `already_processed`. A gap → sync error; the outbox stays pending.
5. `payment.captured` is sequence N and does not close the sale. `order.paid` is sequence N+1 and atomically inserts the captured payment, stores the immutable receipt/tax snapshot, and marks the order paid. A drop after N leaves no completed cloud sale.
5. After a gated restore or reconnect from a restored DB: **pull before push**.

Event catalog (frozen): `contracts/events.json`. Schema parity contract: `contracts/schema-contract.json`.

Parallel migrations, no shared ORM:

- SQLite: `src-tauri/migrations/sqlite/`
- Postgres: `supabase/migrations/`
- Same operational table names and money columns. SQLite-only: outbox, sync_state, offline PIN cache, device_sequence. Postgres-only: sync_receipts, expenses (Phase 2), cashier_shifts (Phase 2).

## Money, time, tax

All money is integer minor units (piastres).

Linear gaming charge (integer floor):

`charge_minor = (rate_minor_per_hour × duration_seconds) / 3600`

Canonical paid identity:

`subtotal_minor + tax_minor - discount_minor = total_minor`

`subtotal_minor = product_subtotal_minor + gaming_subtotal_minor`

**P1-4 locked — tax-ready snapshots, tax disabled in MVP**

- Columns exist now: `tax_rate_bps`, `tax_minor`, `subtotal_minor`.
- MVP writes: `tax_rate_bps = 0`, `tax_minor = 0`. Discount is also 0, so `subtotal_minor = total_minor`.
- No tax/VAT configuration UI, no automatic VAT, no VAT line on cashier or customer surfaces.
- Do not implement Egyptian VAT or assume VAT registration.
- Receipt snapshot stores the exact tax values from payment time.
- Paid tax fields are immutable. Sync replay copies the snapshot and must not recalculate tax from a rate.

Timestamps are UTC. Branch timezone is `Africa/Cairo` for display and report day bounds. Timers use `started_at`, never `seconds++`. Duration is never negative. Clock jumps are flagged.

## Payments

Cash only in MVP. One captured sale payment per order. Paid financial rows are immutable.

`reverse_payment` is in MVP: original payment row stays (`reversed`); order returns to `checkout_pending`; audit + outbox. Full refunds are Phase 2.

Receipt snapshot is stored at payment. Thermal printing is Phase 2.

## Restore

Gated. A cloud-ahead backup must not silently become the live local DB. Offline emergency: newest verified local backup. After reconnect: pull before push. `VACUUM INTO` + `integrity_check`.

## Auth

Supabase Auth online. Tokens stay in Rust (never the webview). Offline PIN: Argon2id hash only, expires 72 hours after last online auth. Admin/config changes require internet.

## Environments

Local Supabase CLI for development. One hosted Pro project for production. Debug builds never point at production.

## Phase 2 (not MVP)

Thermal/ESC-POS, expenses, full refunds, shifts, card payments, barcode, stock transfers, images, promotions, customer accounts.

## Definition of Done (MVP)

- Cashier can start/stop/resume stations, sell products, take cash, reverse a payment (admin), and see today’s Cairo sales.
- Offline PIN unlock works for 72 hours after last online auth.
- Outbox + strict sequence + idempotent `apply_domain_event`.
- Tax snapshots exist, default to zero, and survive replay without recalculation.
- Gated backup/restore with pull-before-push.
- Contract tests for the event catalog and schema/tax parity.
- Domain tests for money, pricing floor, clock, and tax identity.
- Windows packaging when the MSVC C++ workload is available.
