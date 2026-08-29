# Release-candidate readiness

Status date: 2026-08-29. Update the packaging row after the NSIS workflow finishes.

Baseline accepted by operator: GitHub Actions run `33245609614` at `36a2337` (contracts, cafe-domain, postgres, tauri-windows).

## Green / proven

| Area | Evidence |
| --- | --- |
| SQLite empty-DB migrate + identities + integrity | `tauri-windows` on `36a2337` and later |
| Postgres `payment.captured` / `order.paid` atomicity + RLS | `postgres` job |
| Event contracts, tax snapshots, webview has no supabase-js | `contracts` job |
| Domain pricing / apply rules | `cafe-domain` job |
| Sync outbox, sequence, restart, pull-before-push | Rust tests in `src-tauri/tests` |
| Offline PIN 72h, clock anomaly, inventory, reverse payment | Rust tests |
| Reports identity, RTL contract, ops restore gate | Rust + vitest |
| Token refresh on 401, sanitized auth/RPC errors | `src-tauri/src/sync/engine.rs`, `supabase_auth.rs` |
| Release CSP overlay, localhost/prod env split, service-role reject | `tauri.release.conf.json`, `resolve_supabase_config` |
| Debug seed gated in release | `seed_dev_data` + UI `health.debug` |
| Backup retention (14) | `backup::prune_backups` |
| HTTP timeouts (15s / 8s connect) | `transport::http_client` |
| Dedicated unsigned NSIS workflow + validation + smoke | `.github/workflows/package-windows.yml` |
| Build artifacts and signing material stay out of git | `.gitignore` |

Packaging artifact (fill after dispatch):

| Field | Value |
| --- | --- |
| Workflow run | _pending first dispatch_ |
| Packaged SHA | _pending_ |
| Installer | _pending_ |
| SHA-256 | _pending_ |
| Signing | unsigned (expected) |

## External / manual remaining

- Windows Authenticode certificate (unsigned installer is not production-distributable)
- Real production Supabase project URL + anon key (never commit; never service-role)
- Physical two-branch writer-PC deployment
- Final cashier/admin UAT on a clean Windows install ([acceptance-checklist.md](acceptance-checklist.md))
- Hardware peripherals if added later

Do not treat local missing `link.exe` as a packaging blocker. GitHub Actions Windows already compiles the Tauri crate and is the installer factory.
