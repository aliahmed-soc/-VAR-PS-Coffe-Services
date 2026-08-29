# Architecture decisions (frozen)

Source of truth for implementation. Do not reopen these without an explicit product change.

## Stack

- Tauri 2 + React + TypeScript + Tailwind CSS
- Rust owns SQLite, domain operations, pricing, auth tokens, Supabase HTTP, sync, backup
- React is UI only
- Local: SQLx + SQLite (WAL, foreign keys)
- Cloud: one Supabase Pro project (Postgres, Auth, RLS, RPC)
- No custom API server, no Prisma, no Drizzle, no `supabase-js` in the webview
- One transaction-writing cashier PC per branch (MVP)
- License: MIT

## Money and time

- Store money as integer minor units (piastres). Never `f64` / `f32` for money.
- Linear: `charge_minor = (rate_minor_per_hour * duration_seconds) / 3600` integer floor
- One Rust pricing module. No UI pricing math.
- Timestamps UTC. Branch timezone `Africa/Cairo` for display and report bounds.
- Timer uses `started_at`, not `seconds++`.
- Duration never negative. Clock jumps are flagged. Admin correction is audited.
- Snapshots: `tax_minor = 0`, `tax_rate_bps = 0` in MVP. No tax UI.

## Sync

- Domain events with UUID `event_id`
- Local transactional outbox in the same SQLite transaction as business writes
- Strict per-device `local_sequence`; server accepts only `last + 1`
- Duplicate `event_id` → already processed
- Sequence gap → sync error, outbox stays pending
- Rust sync worker is independent of the webview
- Pull-before-push after restore / reconnect from a restored DB

## Payments

- Cash only in MVP
- One captured sale payment per order
- Paid rows are immutable
- `reverse_payment` is in MVP: original payment stays, order → `checkout_pending`, audit + outbox
- Full refunds are Phase 2

## Restore

- Gated. Cloud-ahead must not silently become the live local DB
- Offline emergency: newest verified local backup
- After reconnect: pull before push

## Auth

- Supabase Auth online; tokens stored by Rust (OS credential store)
- Offline PIN: Argon2id hash only, expires 72 hours after last online auth
- Admin/config changes require internet

## Environments

- Local Supabase CLI for development
- One hosted Pro project for production
- Debug builds never point at production

## Phase 2 (not on MVP path)

Thermal/ESC-POS, expenses, full refunds, shifts, card payments, barcode, stock transfers, images, promotions, customer accounts
