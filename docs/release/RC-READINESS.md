# Release-candidate readiness

Status date: 2026-08-30.

Documented HEAD: `6723890fd56d608c846c45c44a80fbf58636e444`

Packaged installer HEAD (not rebuilt): `ede077a25e6a06f1175e0254f7145b68f9d17569`

Release builds default the cloud URL to `https://rbxtxtlssknjioaveytg.supabase.co`.
Hosted migrations `20260829000100` / `20260829000200` / `20260829000300` remain applied.
Re-verified 2026-08-30 via session pooler: `inspect_remote.sql` and `validate_schema.sql` passed; `auth.users` = 0; no HT1/HT2/HTE leftovers; no hosted-test helper functions.
The publishable key is still not available to compile in. Auth users are not created yet.
`SUPABASE_ACCESS_TOKEN` is still absent, so **Hosted Supabase migrate** has never been dispatched.

## Green / proven

| Area | Evidence |
| --- | --- |
| contracts, cafe-domain, postgres, tauri-windows | [CI 33274654994](https://github.com/aliahmed-soc/-VAR-PS-Coffe-Services/actions/runs/33274654994) on `6723890` |
| Hosted schema / grants / indexes / linear CHECK / tax trigger | `validate_schema.sql` on `rbxtxtlssknjioaveytg` (re-verified 2026-08-30) |
| Hosted RLS matrix (anon / HT1 / HT2 / inactive / admin) | `rls_matrix.sql` disposable IDs only (prior pass; leftovers absent on re-verify) |
| Hosted `apply_domain_event` (linear, captured≠paid, paid atomic, reverse) | `event_acceptance.sql` disposable `HTE` only (prior pass; leftovers absent on re-verify) |
| Unsigned NSIS installer built, validated, smoked, uploaded | [Package Windows NSIS 33272754059](https://github.com/aliahmed-soc/-VAR-PS-Coffe-Services/actions/runs/33272754059) on `ede077a` |
| Installer file | `PlayStation Cafe POS_0.1.0_x64-setup.exe` (4,430,819 bytes) |
| SHA-256 | `cdeda6354722cbb1b445c1058fb80b9e9fa1aec0507a5cfee4eb16d95469acc8` |
| Artifact name | `playstation-cafe-nsis-unsigned-ede077a` |
| Compile-time publishable key | absent (runtime/GHA secret `PSC_SUPABASE_ANON_KEY` still required) |
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
- Supabase CLI personal access token as GitHub secret `SUPABASE_ACCESS_TOKEN` — required for the **Hosted Supabase migrate** workflow (`workflow_dispatch`); production schema was applied with the database password instead
- Production publishable key (`sb_publishable_...`) in cashier env or GitHub secret `PSC_SUPABASE_ANON_KEY` — UAT NSIS rebuild with `require_publishable_key=true` is blocked until this secret exists
- Rotate the exposed `sb_secret_...` in the Supabase dashboard
- Create Auth users in the dashboard (email + password), then fill `supabase/bootstrap/production.sql.template` — login/refresh/PIN/72h not exercised; no production Auth users exist
- Branch display names, station codes, device keys, linear hourly rates, and later product/inventory load
- Physical two-branch writer-PC deployment
- Final cashier/admin UAT on a clean Windows install ([acceptance-checklist.md](acceptance-checklist.md))
