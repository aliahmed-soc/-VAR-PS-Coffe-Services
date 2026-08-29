# External blockers

Items that cannot be fully finished in this development environment. Work continues around them.

## MSVC linker (`link.exe`)

- **Status:** Visual Studio Build Tools 2022 is installed, but the C++ (`VCTools`) workload is missing `link.exe`.
- **Impact:** `cargo test` for the Tauri crate and `tauri build` need a C linker for bundled SQLite.
- **Mitigation:** Install/repair `Microsoft.VisualStudio.Workload.VCTools`. Domain tests run with `cargo test -p cafe-domain` (18 passing).
- **Not a stop:** schemas, event catalog, cashier UI, migrations, and cafe-domain tests are implemented.

## Hosted Supabase Pro project

- **Status:** No production project credentials in this environment (correct).
- **Mitigation:** Local Supabase CLI + `.env.example`. Production URL/keys are operator-only.

## Thermal printer / physical cash drawer

- Phase 2. Not required for MVP.
