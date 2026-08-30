# Final human acceptance checklist

Automated coverage is in GitHub Actions (four required CI jobs + Package Windows NSIS). Hosted Auth login/isolation/refresh for disposable UAT users is already proven from this workspace. This list is only what still needs a physical Windows machine.

## Package (do not rebuild)

- Commit: `24c0bf349b9a2a9a437811df1f8944ec4fd006c7`
- Artifact: `playstation-cafe-nsis-unsigned-24c0bf3`
- File: `PlayStation Cafe POS_0.1.0_x64-setup.exe`
- SHA-256: `28c847a3b302969deab9cdb579ea0bfe9b5dfd8721d69c83de3be1093c7cf63c`
- Run: https://github.com/aliahmed-soc/-VAR-PS-Coffe-Services/actions/runs/33324667053
- Local copy (if present): `C:\Users\ali_n\.playstation-cafe-uat\installer\`
- UAT emails/passwords/device keys: `C:\Users\ali_n\.playstation-cafe-uat\credentials.json` (not in git)

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
