# Implementation checkpoint

Last updated: 2026-08-29

## Status

Architecture frozen. Foundation and core domain transactions are implemented in-tree.

## Done

- Tauri 2 + React + TypeScript + Tailwind + i18n (en/ar, RTL)
- Frozen decisions in `docs/architecture/DECISIONS.md`
- Shared event catalog `contracts/events.json` + Vitest contract tests (passing)
- Parallel SQLite and Postgres migrations
- RLS policies and `apply_domain_event` / `pull_branch_since` RPCs
- Rust domain: money, pricing (integer floor), clock, gaming, inventory, orders, payments, reverse_payment
- Transactional outbox + per-device `local_sequence`
- Sync worker (Tokio, independent of React) + Rust Supabase transport
- Offline PIN (Argon2id, 72h) + online password login via Rust HTTP
- Backup via SQLite `VACUUM INTO` + integrity_check
- Local sales report
- Cashier UI: stations, start/stop, add Coke, cash pay, reverse
- Integration test `src-tauri/tests/integrity.rs` (needs Rust linker)

## Environment notes

- Parent folder `PS & Coffe Services` contains `&`, which breaks `npm.cmd`. Use `scripts/test.ps1` or `node .\node_modules\vitest\vitest.mjs run`.
- VS Build Tools 2022 is present but **without** the C++ workload (`link.exe` missing). WinLibs MinGW is being installed so `cargo +gnu test` can run. `tauri build` still needs MSVC VCTools.

## Tests run this session

- `vitest` event-catalog contract: 3 passed
- `cargo +gnu test -p cafe-domain`: 18 passed (money, linear 52.75 EGP, stepped, clock, catalog)

## Next

1. Install VS Build Tools C++ workload + WebView2 so `tauri dev` and SQLite integration tests run
2. Set git `user.name` / `user.email` (agent cannot write git config) and commit
3. Prefer a workspace path without `&` (this folder name breaks npm.cmd and windres)
4. Local Supabase CLI apply + RLS live tests
5. Finish reference-data pull UI, restore UI, signed installer
