# Release configuration

## Cloud access

- Rust is the only process that speaks to Supabase (`reqwest`, 15s timeout / 8s connect).
- The webview has no `supabase-js` and no service-role material.
- `PSC_SUPABASE_URL` + `PSC_SUPABASE_ANON_KEY` only. `SUPABASE_SERVICE_ROLE_KEY` is never packaged and is rejected if it equals the configured anon key.
- JWT payloads with `role=service_role` are rejected.
- Release builds reject loopback URLs (`localhost`, `127.0.0.1`, `[::1]`).
- Debug builds reject hosted `*.supabase.co` unless `PSC_ALLOW_PROD=1`.
- `seed_dev_data` is compiled out of the release command surface (`Forbidden` in release; UI hidden unless `app_health.debug`).

## CSP

- Dev (`tauri.conf.json`): allows Vite HMR on ports 1420/1421.
- Release overlay (`tauri.release.conf.json`): `connect-src 'self'` only. No localhost Vite.

## Capabilities

`src-tauri/capabilities/default.json` is `core:default` only. Privileged secrets stay in the Rust session store, not in invoke results.

## Logs

Login/refresh/RPC failures do not append response bodies (those can contain tokens or emails). Sync errors keep `sequence_gap` / `event_id_payload_mismatch` codes only.

## Identity and restore across upgrades

- `device_id` is a file next to `branch.sqlite` in `app_data_dir`.
- Installer upgrades overwrite program files, not AppData.
- Staged restore writes `branch.sqlite.restore` and sets `restore_reconciliation_required` so the next start must pull-before-push.

## Backups

Directory: `{app_data_dir}/backups`. Newest 14 `*.sqlite` files are retained; older files are deleted after a successful verified backup.
