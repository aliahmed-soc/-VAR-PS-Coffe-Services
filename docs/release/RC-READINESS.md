# Release-candidate readiness

Status date: 2026-08-29.

Packaged HEAD: `ede077a25e6a06f1175e0254f7145b68f9d17569`

Release builds default the cloud URL to `https://rbxtxtlssknjioaveytg.supabase.co`.
The publishable key was not available to compile in. Hosted migrations are not applied yet.

## Green / proven

| Area | Evidence |
| --- | --- |
| contracts, cafe-domain, postgres, tauri-windows | [CI 33272630590](https://github.com/aliahmed-soc/-VAR-PS-Coffe-Services/actions/runs/33272630590) on `ede077a` |
| Unsigned NSIS installer built, validated, smoked, uploaded | [Package Windows NSIS 33272754059](https://github.com/aliahmed-soc/-VAR-PS-Coffe-Services/actions/runs/33272754059) |
| Installer file | `PlayStation Cafe POS_0.1.0_x64-setup.exe` (4,430,819 bytes) |
| SHA-256 | `cdeda6354722cbb1b445c1058fb80b9e9fa1aec0507a5cfee4eb16d95469acc8` |
| Artifact name | `playstation-cafe-nsis-unsigned-ede077a` |
| Compile-time publishable key | absent (runtime/GHA secret still required) |
| Installer secret scan | no `sb_secret_`, PEM, or service-role material |
| Signing | unsigned (expected; not a build failure) |
| MVP gaming charge | `floor(rate_minor_per_hour * actual_duration_seconds / 3600)` only |
| Stepped / increment cannot charge | cafe-domain tests; SQLite `CHECK (rule_type = 'linear')`; Postgres same + `mvp_linear_pricing_required` |
| Session snapshot immutable vs later rate | `src-tauri/tests/pricing.rs` |
| UI has no pricing math | `tests/contract/pricing.test.ts` |
| Clean install ships no DB/secrets | `scripts/smoke-nsis-install.ps1` |
| Upgrade preserves adjacent SQLite marker | same smoke |
| Uninstall does not wipe business-data marker | same smoke |
| Release CSP without Vite localhost | `src-tauri/tauri.release.conf.json` |
| Debug seed gated; localhost/prod env split; service-role rejected | `seed_dev_data`, `resolve_supabase_config` |
| Token refresh on 401; sanitized auth/RPC errors | `engine.rs`, `supabase_auth.rs` |
| HTTP timeouts 15s / 8s connect | `transport::http_client` |
| Backup retention 14 | `backup::prune_backups` |
| Build artifacts and cert material stay out of git | `.gitignore` |

Frozen architecture (sync, payment, tax, RLS, restore, offline-auth, inventory, RTL, integrity, linear pricing) was not reopened beyond conforming the implementation to the already-frozen formula.

Ordinary MVP feature work is stopped.

## External / manual remaining

- Windows Authenticode certificate (unsigned installer is not production-distributable)
- Supabase CLI personal access token (`SUPABASE_ACCESS_TOKEN`) or hosted Postgres password — required to apply `supabase/migrations/` to `rbxtxtlssknjioaveytg` (the desktop publishable key cannot do this)
- Production publishable key (`sb_publishable_...`) in cashier env or GitHub secret `PSC_SUPABASE_ANON_KEY` — not present in this workspace
- Rotate the exposed `sb_secret_...` in the Supabase dashboard
- Auth user emails/passwords, branch display names, station codes, device keys, linear hourly rates, and later product/inventory load
- Physical two-branch writer-PC deployment
- Final cashier/admin UAT on a clean Windows install ([acceptance-checklist.md](acceptance-checklist.md))
