# Release-candidate readiness

Status date: 2026-08-30.

Packaged HEAD: `24c0bf349b9a2a9a437811df1f8944ec4fd006c7`

Release builds default the cloud URL to `https://rbxtxtlssknjioaveytg.supabase.co`.
Hosted migrations `20260829000100` / `20260829000200` / `20260829000300` remain applied and accepted.
This UAT installer was compiled with repository secret `PSC_SUPABASE_ANON_KEY` (publishable only; value not printed). Elevated `sb_secret_` / `service_role` keys are rejected by the package workflow.

## Green / proven

| Area | Evidence |
| --- | --- |
| contracts, cafe-domain, postgres, tauri-windows | [CI 33322833158](https://github.com/aliahmed-soc/-VAR-PS-Coffe-Services/actions/runs/33322833158) on `24c0bf3` |
| Hosted schema / RLS / `apply_domain_event` | [Hosted Supabase migrate 33322621211](https://github.com/aliahmed-soc/-VAR-PS-Coffe-Services/actions/runs/33322621211) (accepted) |
| Unsigned UAT NSIS built, validated, smoked, uploaded | [Package Windows NSIS 33324667053](https://github.com/aliahmed-soc/-VAR-PS-Coffe-Services/actions/runs/33324667053) |
| Packaged commit | `24c0bf349b9a2a9a437811df1f8944ec4fd006c7` (log: `Packaging commit 24c0bf349b9a2a9a437811df1f8944ec4fd006c7`) |
| Contract / domain tests in package job | vitest 9 files / 30 passed; cafe-domain 40 passed |
| Elevated-key guard | `require_publishable_key=true`; log: `Publishable key present for compile-time release config (value not printed)`; secret masked as `***` |
| Installer file | `PlayStation Cafe POS_0.1.0_x64-setup.exe` (4,431,936 bytes) |
| SHA-256 | `28c847a3b302969deab9cdb579ea0bfe9b5dfd8721d69c83de3be1093c7cf63c` |
| Artifact name | `playstation-cafe-nsis-unsigned-24c0bf3` |
| Installer secret scan | `secret_string_hits: []` — no `sb_secret_`, PEM/private-key, service-role material, DB, or `.env` |
| Install / upgrade / uninstall smoke | passed (no DB on clean install; upgrade preserves marker; uninstall does not wipe data) |
| Signing | unsigned (expected; not a build failure) |
| Compile-time publishable key | present (not printed) |
| MVP gaming charge | `floor(rate_minor_per_hour * actual_duration_seconds / 3600)` only |

Ordinary MVP feature work is stopped. No GitHub Release was published.

## External / operational remaining

- Create real Supabase Auth users in the dashboard
- Bootstrap branches, users, roles, stations, device identities, and linear pricing (`supabase/bootstrap/production.sql.template`)
- Hosted Auth login + refresh validation
- Clean-Windows physical cashier/admin UAT ([acceptance-checklist.md](acceptance-checklist.md))
- Windows Authenticode signing after UAT approval
- Rotate/revoke the previously leaked `sb_secret_...` in the Supabase dashboard (unused by this installer)
