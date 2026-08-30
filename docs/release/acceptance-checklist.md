# Final human acceptance checklist

Automated coverage is in GitHub Actions (four required CI jobs + Package Windows NSIS). Hosted Auth login/isolation/refresh for disposable UAT users is already proven from this workspace. This list is only what still needs a physical Windows machine.

## Package

`24c0bf3` was superseded: clean first-run online reference bootstrap was missing.
`da192b3` was superseded too: the `PSC_SUPABASE_ANON_KEY` build secret held a legacy
`service_role` JWT, so the runtime correctly refused it and every hosted login failed
with `auth: cloud not configured`. `d489f70` carried phases A–G of physical UAT and was
then superseded by the receipt-numbering fix. Use the newest unsigned UAT installer
recorded in `RC-READINESS.md`.

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

- [ ] Clean install of the SHA above
- [ ] First launch; `branch.sqlite` + `device_id` created
- [ ] B1 cashier online login (credentials file)
- [ ] Offline PIN creation
- [ ] Start a PS station
- [ ] Add UAT products
- [ ] Disconnect network
- [ ] Continue locally
- [ ] Stop session
- [ ] Cash checkout
- [ ] Restart application
- [ ] Paid order/receipt persists
- [ ] Payment reversal
- [ ] Corrected repayment
- [ ] Reconnect
- [ ] Sync drains in strict sequence
- [ ] No duplicate order/payment/receipt
- [ ] B2 remains isolated
- [ ] Branch reports correct
- [ ] Arabic RTL usable
- [ ] Backup
- [ ] Restore
- [ ] Pull-before-push
- [ ] Stale restore cannot overwrite cloud
- [ ] Kill/restart UI during sync
- [ ] Sync worker recovers
- [ ] 72-hour offline PIN boundary (lab clock only)
- [ ] Installer upgrade preserves local database/device identity

After acceptance, run `supabase/bootstrap/uat_cleanup.sql` once. Do not run it now.

## Explicitly not this checklist

Do not re-decide architecture, sync, payment, tax, RLS, restore, offline-auth, inventory, RTL, or integrity. Those are frozen and already covered by CI. Authenticode waits until this list passes.
