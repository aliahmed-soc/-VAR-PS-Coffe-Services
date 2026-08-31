# Final human acceptance checklist

Automated coverage is in GitHub Actions (four required CI jobs + Package Windows NSIS). Hosted Auth login/isolation/refresh for disposable UAT users is already proven from this workspace. This list is only what still needs a physical Windows machine.

## Package

`24c0bf3` was superseded: clean first-run online reference bootstrap was missing.
`da192b3` was superseded too: the `PSC_SUPABASE_ANON_KEY` build secret held a legacy
`service_role` JWT, so the runtime correctly refused it and every hosted login failed
with `auth: cloud not configured`. `d489f70` carried phases A–G of physical UAT and was
then superseded by the receipt-numbering fix, `e7cd260` by the sync-badge fix, `bc22240`
by the restore WAL/sequence fixes, and `94c986d` by the ticket-void inventory fix.

**Accepted build: `0a85f7eace1d222c49be5db48ed4528789bbdd33`**, installer SHA-256
`0511b3336d710e73121b235da491eda2acfaa540ea20a3f2fdc0a3efcaee4efb`. Every physical item
below passed on it. Details of each defect are in `RC-READINESS.md`.

- UAT emails/passwords: `C:\Users\ali_n\.playstation-cafe-uat\credentials.json` (not in git)

Writer-device provisioning (do this before any sale):

1. Clean install and first launch on the B1 PC (creates `{app_data_dir}/device_id`).
2. Close the app. Do not start a session or take payment.
3. Read that file. Register it as the UAT1 cloud writer via the privileged SQL path.
4. Repeat independently for UAT2 on a second PC.

Windows SmartScreen / “unknown publisher” is expected until Authenticode exists.

## Disposable UAT identities (not production employees)

- Admin: `uat-admin@invalid.test`
- B1 cashier: `uat-b1-cashier@invalid.test` (UAT1 only)
- B2 cashier: `uat-b2-cashier@invalid.test` (UAT2 only)
- Branches: `UAT1` / `UAT2` (`Africa/Cairo`, `EGP`)
- Stations: PS1–PS5 per UAT branch
- Pricing: linear **UAT-only** 3000 minor/hour — replace before live trading
- Products: `UAT-DRINK`, `UAT-SNACK`

## Physical Windows run

Evidence lives outside git in `C:\Users\ali_n\.playstation-cafe-uat\evidence` (screenshots)
alongside the local-SQLite and hosted-SQL readings quoted in the UAT report.

- [x] Clean install of the SHA above
- [x] First launch; `branch.sqlite` + `device_id` created
- [x] B1 cashier online login (credentials file)
- [x] Offline PIN creation
- [x] Start a PS station
- [x] Add UAT products
- [x] Disconnect network
- [x] Continue locally
- [x] Stop session
- [x] Cash checkout
- [x] Restart application
- [x] Paid order/receipt persists
- [x] Payment reversal
- [x] Corrected repayment
- [x] Reconnect
- [x] Sync drains in strict sequence
- [x] No duplicate order/payment/receipt
- [x] B2 remains isolated
- [x] Branch reports correct
- [x] Arabic RTL usable
- [x] Backup
- [x] Restore
- [x] Pull-before-push
- [x] Stale restore cannot overwrite cloud
- [x] Kill/restart UI during sync
- [x] Sync worker recovers
- [x] 72-hour offline PIN boundary (lab clock only)
- [x] Installer upgrade preserves local database/device identity
- [x] Voiding a ticket returns its lines to stock (added after the phase C defect)

Deviation from the plan: no second physical PC was available, so UAT2 ran as an isolated
second Windows profile with its own app-data directory, its own `device_id`, and its own
registered cloud writer. Windows Sandbox could not start in this disconnected RDP session.

After acceptance, run `supabase/bootstrap/uat_cleanup.sql` once. Do not run it now.

## Explicitly not this checklist

Do not re-decide architecture, sync, payment, tax, RLS, restore, offline-auth, inventory, RTL, or integrity. Those are frozen and already covered by CI. Authenticode waits until this list passes.
