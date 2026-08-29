# Final human acceptance checklist

Automated coverage is in GitHub Actions (four required CI jobs + Package Windows NSIS). This list is only what still needs a person or a physical/hosted environment.

## Before UAT

- [ ] Download the unsigned NSIS artifact from the packaging run that matches the green SHA.
- [ ] Confirm SHA-256 against `nsis-sha256.txt`.
- [ ] Windows SmartScreen / “unknown publisher” is expected until Authenticode exists.

## Clean install (physical PC or full desktop VM)

- [ ] Install on a clean Windows 10/11 machine.
- [ ] First launch creates `{app_data_dir}/branch.sqlite` and `device_id` only after start (empty DB + migrations).
- [ ] Online login against the **intended** environment (local CLI or future hosted project).
- [ ] Cache offline PIN; start a PS station; add products.
- [ ] Disconnect NIC; continue; stop session; checkout gaming + products; take cash.
- [ ] Quit and restart; paid order/receipt still present.
- [ ] Reverse payment; repay; reconnect; strict ordered sync; no duplicate payment/order/receipt.
- [ ] Branch B2 unchanged.
- [ ] Branch reports match paid identities.
- [ ] Arabic RTL layout usable at the cashier desk.
- [ ] Create backup; stage restore; confirm pull-before-push; stale restore cannot overwrite newer cloud.
- [ ] Kill/restart the window while sync is active; worker recovers (no second writer).
- [ ] Install a newer NSIS build over the first; `branch.sqlite` and `device_id` remain valid.

## Offline boundary

- [ ] Confirm 72-hour PIN expiry forces online login (can be clock-adjusted in a lab; do not do this on a live branch).

## Explicitly not this checklist

Do not re-decide architecture, sync, payment, tax, RLS, restore, offline-auth, inventory, RTL, or integrity. Those are frozen and already covered by CI.
