# Release-candidate readiness

Status date: 2026-08-29.

Packaged HEAD: `5d8165b17afc2902dd7903e431ad822272fa2c26`

Operator-accepted prior baseline: run `33245609614` at `36a2337`.
This packaging commit also has a full green CI run.

## Green / proven

| Area | Evidence |
| --- | --- |
| contracts, cafe-domain, postgres, tauri-windows | [CI 33251760125](https://github.com/aliahmed-soc/-VAR-PS-Coffe-Services/actions/runs/33251760125) on `5d8165b` |
| Unsigned NSIS installer built, validated, smoked, uploaded | [Package Windows NSIS 33251765818](https://github.com/aliahmed-soc/-VAR-PS-Coffe-Services/actions/runs/33251765818) |
| Installer file | `PlayStation Cafe POS_0.1.0_x64-setup.exe` (4,445,027 bytes) |
| SHA-256 | `02a2f2c9ac8975ce665e85d0fd9aa2d4ca0e207b5d23cc7afdc8c7a90a50c4df` |
| Artifact name | `playstation-cafe-nsis-unsigned-5d8165b` |
| Signing | unsigned (expected; not a build failure) |
| Clean install ships no DB/secrets | `scripts/smoke-nsis-install.ps1` |
| Upgrade preserves adjacent SQLite marker | same smoke |
| Uninstall does not wipe business-data marker | same smoke |
| Release CSP without Vite localhost | `src-tauri/tauri.release.conf.json` |
| Debug seed gated; localhost/prod env split; service-role rejected | `seed_dev_data`, `resolve_supabase_config` |
| Token refresh on 401; sanitized auth/RPC errors | `engine.rs`, `supabase_auth.rs` |
| HTTP timeouts 15s / 8s connect | `transport::http_client` |
| Backup retention 14 | `backup::prune_backups` |
| Build artifacts and cert material stay out of git | `.gitignore` |

Frozen architecture (sync, payment, tax, RLS, restore, offline-auth, inventory, RTL, integrity) was not reopened.

## External / manual remaining

- Windows Authenticode certificate (unsigned installer is not production-distributable)
- Real production Supabase project URL + anon key (never commit; never service-role)
- Physical two-branch writer-PC deployment
- Final cashier/admin UAT on a clean Windows install ([acceptance-checklist.md](acceptance-checklist.md))
- Hardware peripherals if added later

Local missing `link.exe` is **not** a packaging blocker. GitHub Actions Windows is the installer factory.
