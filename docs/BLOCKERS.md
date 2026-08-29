# External blockers

Items that cannot be fully finished in this development environment. Work continues around them.

## MSVC linker (`link.exe`)

- **Status:** Visual Studio Build Tools 2022 is installed, but the C++ (`VCTools`) workload did not appear on disk when implementation started.
- **Impact:** `cargo test` / `tauri build` need a C linker to compile `libsqlite3-sys` (bundled SQLite) and Tauri.
- **Mitigation:** Keep installing/repairing `Microsoft.VisualStudio.Workload.VCTools`. All domain code and SQL are written so tests run as soon as the linker exists.
- **Not a stop:** schemas, event catalog, UI, migrations, and tests are implemented regardless.

## Hosted Supabase Pro project

- **Status:** No production project credentials in this environment (correct).
- **Mitigation:** Local Supabase CLI + `.env.example`. Production URL/keys are operator-only and never shipped in the desktop binary except the publishable URL + anon key for the chosen environment.

## Git author identity

- **Status:** `git commit` failed because this machine has no `user.name` / `user.email`. This agent is not allowed to write git config.
- **Action required on the machine:** set local repo identity, then commit. The working tree is ready.

## Thermal printer / physical cash drawer

- Phase 2. Not required for MVP.
