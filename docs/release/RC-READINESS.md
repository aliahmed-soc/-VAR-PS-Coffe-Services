# Release-candidate readiness

Status date: 2026-08-30.

Packaged HEAD: `da192b3f89ad6c780228471383f810e821736fed`

`24c0bf349b9a2a9a437811df1f8944ec4fd006c7` was superseded: a clean install could authenticate with Supabase but could not resolve a branch because `login_online` read empty local `user_branch_roles`. First-run now downloads RLS-visible reference data in Rust, caches it transactionally, then creates the session and offline PIN.

`da192b3f89ad6c780228471383f810e821736fed` was superseded during physical GUI UAT. The
`PSC_SUPABASE_ANON_KEY` build secret held the project's legacy `service_role` JWT rather
than the publishable key. The pre-compile guard only substring-matched `service_role`,
which a base64url JWT payload never contains, so the elevated key was compiled into the
installer. The desktop runtime refused it exactly as designed (`elevated_key_forbidden`),
which surfaced as `auth: cloud not configured` on every GUI login, blocking UAT.

Remediation: the build secret now holds the publishable key; the project's legacy JWT API
keys are disabled, so the exposed `service_role` JWT is dead (`401`); the pre-compile guard
and the artifact validator now decode JWT role claims (`scripts/cloud-key-guard.mjs`,
covered by `tests/contract/cloud-key-guard.test.ts`); the two installer artifacts that
embedded the key were deleted.

Hosted migrations `20260829000100` / `20260829000200` / `20260829000300` remain applied.
UAT Auth users and UAT1/UAT2 reference data remain. Disposable API-acceptance orders/receipts/sequences were reset; inventory baseline is 20.

## Green / proven

| Area | Evidence |
| --- | --- |
| contracts, cafe-domain, postgres, tauri-windows | [CI 33331579343](https://github.com/aliahmed-soc/-VAR-PS-Coffe-Services/actions/runs/33331579343) on `da192b3` |
| First-login bootstrap tests | `src-tauri/tests/first_login.rs`, `first_login_repro.rs` |
| Unsigned UAT NSIS | [Package Windows NSIS 33331780899](https://github.com/aliahmed-soc/-VAR-PS-Coffe-Services/actions/runs/33331780899) |
| Packaged commit | `da192b3f89ad6c780228471383f810e821736fed` |
| Publishable key in logs | present, value not printed (`***`) |
| Installer file | `PlayStation Cafe POS_0.1.0_x64-setup.exe` (4,460,857 bytes) |
| SHA-256 | `17ae387af9d1d6f40ebf1c9f90c3dc832c5ae5abacb094f1fd9f9cbff59eb289` |
| Artifact name | `playstation-cafe-nsis-unsigned-da192b3` |
| Installer secret scan | `secret_string_hits: []` |
| Install / upgrade / uninstall smoke | passed |
| Signing | unsigned (expected) |

Ordinary MVP feature work is stopped. No GitHub Release. Authenticode waits for physical UAT.

## External / operational remaining

- Physical clean-Windows UAT of **this** installer (not `24c0bf3`), including writer `device_id` registration
- Authenticode after UAT
- Replace UAT-only 3000 minor/hour rate and UAT products before live trading
