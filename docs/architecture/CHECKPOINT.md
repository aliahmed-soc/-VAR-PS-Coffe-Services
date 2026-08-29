# Implementation checkpoint

Last updated: 2026-08-29

## Status

Architecture frozen. Cashier workspace, domain transactions, and GitHub origin are in place.

Remote: https://github.com/aliahmed-soc/-VAR-PS-Coffe-Services

## Done

- Tauri 2 + React + TypeScript + Tailwind + i18n (en/ar, RTL, language persisted)
- Frozen decisions in `docs/architecture/DECISIONS.md`
- Shared event catalog `contracts/events.json` + Vitest contract tests
- Parallel SQLite and Postgres migrations
- RLS policies and `apply_domain_event` / `pull_branch_since` RPCs
- Rust domain: money, pricing (integer floor), clock, gaming, inventory, orders, payments, reverse_payment
- Transactional outbox + per-device `local_sequence`
- Sync worker (Tokio) + Rust Supabase transport
- Offline PIN (Argon2id, 72h) + online password login via Rust HTTP
- Gated backup (`VACUUM INTO`) and restore (stage → restart → pull-before-push)
- Today-in-Cairo sales report (bounds computed in Rust)
- Cashier UI: stations with checkout/resume, walk-in POS, product picker, ticket panel, sales, backup
- GitHub `main` tracks `origin`

## Environment notes

- Parent folder `PS & Coffe Services` contains `&`, which breaks `npm.cmd`. Use `scripts/test.ps1` or `node .\node_modules\vitest\vitest.mjs run`.
- VS Build Tools 2022 is present but **without** the C++ workload (`link.exe` missing). `tauri build` still needs MSVC VCTools.

## Tests run

- `vitest` event-catalog contract: 3 passed
- `cargo +gnu test -p cafe-domain`: 18 passed

## Next

1. Install VS Build Tools C++ workload + WebView2 so `tauri dev` and SQLite integration tests run
2. Local Supabase CLI apply + RLS live tests
3. Signed Windows installer
