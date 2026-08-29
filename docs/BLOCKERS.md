# External blockers

Items that cannot be finished in software alone. Work continues around them.

## Windows Authenticode certificate

- **Status:** No publisher certificate in this environment (correct).
- **Impact:** CI produces an **unsigned** NSIS installer. Windows SmartScreen will warn. Not production-distributable until signed.
- **Not a stop:** Packaging, validation, and smoke install run without a certificate. See `docs/release/signing.md`.

## Hosted Supabase project

- **Status:** No production project credentials in this environment (correct).
- **Mitigation:** Local Supabase CLI + `.env.example`. Production URL + anon key are operator-only. Service-role must never be installed on a cashier PC.

## Physical two-branch UAT / peripherals

- Final cashier/admin acceptance on real writer PCs. Thermal printer / cash drawer are Phase 2.

## Local MSVC (`link.exe`) on a developer PC

- **Not a packaging blocker.** GitHub Actions `windows-latest` has MSVC and already compiles the Tauri crate. The NSIS installer is built there (`Package Windows NSIS` workflow).
