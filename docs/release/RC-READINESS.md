# Release-candidate readiness

Status date: 2026-08-30.

Documented HEAD: `0bc66d9dcc522a54fdf9468fe77f17a04338617a`

Packaged installer HEAD (not rebuilt): `ede077a25e6a06f1175e0254f7145b68f9d17569`

Release builds default the cloud URL to `https://rbxtxtlssknjioaveytg.supabase.co`.
Hosted migrations `20260829000100` / `20260829000200` / `20260829000300` remain applied.
**Hosted Supabase migrate** [33322621211](https://github.com/aliahmed-soc/-VAR-PS-Coffe-Services/actions/runs/33322621211) on `0bc66d9` succeeded: local matched remote for all three versions; dry-run and `db push` both reported the remote database up to date (no-op). `validate_schema.sql` passed. Hosted RLS matrix and `apply_domain_event` acceptance passed; test helpers were dropped. Independent PAT re-query after the run: the same three versions recorded; `auth.users` = 0; no `HT1` / `HT2` / `HTE` leftover branches.
`SUPABASE_ACCESS_TOKEN` is set. `PSC_SUPABASE_ANON_KEY` is still absent, so the unsigned installer was not rebuilt. The leaked project secret was not used and must be rotated in the dashboard immediately.
The publishable key is still not available to compile in. Auth users are not created yet.

## Green / proven

| Area | Evidence |
| --- | --- |
| contracts, cafe-domain, postgres, tauri-windows | [CI 33319723007](https://github.com/aliahmed-soc/-VAR-PS-Coffe-Services/actions/runs/33319723007) on `0bc66d9` |
| Hosted schema / grants / indexes / linear CHECK / tax trigger | [Hosted Supabase migrate 33322621211](https://github.com/aliahmed-soc/-VAR-PS-Coffe-Services/actions/runs/33322621211) `validate_schema.sql` (no-op push; versions re-queried) |
| Hosted RLS matrix (anon / HT1 / HT2 / inactive / admin) | same run, `rls_matrix.sql` disposable IDs only; leftover `HT1`/`HT2` branches = 0 after |
| Hosted `apply_domain_event` (linear, captured≠paid, paid atomic, reverse) | same run, `event_acceptance.sql` disposable `HTE` only; leftover `HTE` branches = 0 after |
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
- Production publishable key (`sb_publishable_...`) via `gh secret set PSC_SUPABASE_ANON_KEY` — UAT NSIS rebuild with `require_publishable_key=true` is blocked until this secret exists; do not send a project secret
- Rotate the exposed `sb_secret_...` in the Supabase dashboard API settings immediately (leaked in chat; unused here)
- Create Auth users in the dashboard (email + password), then fill `supabase/bootstrap/production.sql.template` — login/refresh/PIN/72h not exercised; `auth.users` = 0
- Branch display names, station codes, device keys, linear hourly rates, and later product/inventory load
- Physical two-branch writer-PC deployment
- Final cashier/admin UAT on a clean Windows install ([acceptance-checklist.md](acceptance-checklist.md))
