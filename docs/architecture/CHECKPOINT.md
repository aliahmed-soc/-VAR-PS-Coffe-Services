# Implementation checkpoint

Last updated: 2026-08-29

## Status

Architecture frozen. P0 payment-cloud atomicity is fixed: `payment.captured` does not close a sale; `order.paid` does that in one transaction.

Canonical doc: `ARCHITECTURE.md`.

Remote: https://github.com/aliahmed-soc/-VAR-PS-Coffe-Services

## Done

- Tauri 2 + React + TypeScript + Tailwind + i18n (en/ar, RTL, language persisted)
- Canonical architecture in `docs/architecture/ARCHITECTURE.md` (no `supabase-js` in the webview)
- Frozen decisions in `docs/architecture/DECISIONS.md`
- P1-4 tax-ready snapshots: `tax_minor`/`tax_rate_bps`/`subtotal_minor`, MVP zeros, paid-immutable, replay copies snapshot
- Shared event catalog `contracts/events.json` (frozen) + Vitest contract tests
- Parallel SQLite and Postgres migrations
- RLS policies and `apply_domain_event` / `pull_branch_since` RPCs
- `payment.captured` is sequencing/audit only; `order.paid` atomically inserts the payment, stores the receipt/tax snapshot, and marks the order paid
- Rust domain: money, pricing (integer floor), clock, gaming, inventory, orders, payments, reverse_payment
- Transactional outbox + per-device `local_sequence`
- Sync worker (Tokio) + Rust Supabase transport
- Offline PIN (Argon2id, 72h) + online password login via Rust HTTP
- Gated backup (`VACUUM INTO`) and restore (stage → restart → pull-before-push)
- Today-in-Cairo sales report (bounds computed in Rust)
- Cashier UI: stations with checkout/resume, walk-in POS, product picker, ticket panel, sales, backup
- GitHub Actions CI on `main` (contracts, cafe-domain, Postgres, Windows/MSVC Tauri)
- GitHub `main` tracks `origin`

## Environment notes

- Parent folder `PS & Coffe Services` contains `&`, which breaks `npm.cmd`. Use `scripts/test.ps1` or `node .\node_modules\vitest\vitest.mjs run`.
- VS Build Tools 2022 is present but **without** the C++ workload (`link.exe` missing). Local `tauri` / `integrity.rs` still need MSVC VCTools. CI uses `windows-latest` for that path.

## Tests run this session

- `npx vitest run` (via `node .\node_modules\vitest\vitest.mjs run`): **14 passed** (events, tax, payment-atomicity)
- `cargo test -p cafe-domain`: **37 passed** (includes 14 payment-atomicity / interruption / reverse / sequence tests)
- `cargo clippy -p cafe-domain --all-targets -- -D warnings`: passed
- `cargo fmt -p cafe-domain -- --check`: passed
- `node .\node_modules\typescript\bin\tsc --noEmit`: passed
- PostgreSQL `scripts/run-pg-tests.sh`: not executed on this machine (no local Postgres). Wired for CI.
- `src-tauri` `integrity.rs`: not executed here (MinGW export-ordinal / missing `link.exe`)

## Next

1. Confirm GitHub Actions is green on `main`
2. Live Supabase CLI apply + RLS matrix on a local stack
3. Windows packaging / signed installer after MSVC is available
