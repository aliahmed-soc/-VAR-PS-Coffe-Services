# External dependencies (not ordinary software work)

| Item | Why it is external |
| --- | --- |
| Windows Authenticode certificate | Must be purchased/issued to the legal publisher; not inventable in-repo |
| Hosted production Supabase project | Operator creates the project and injects URL + anon key on the writer PC |
| Physical two-branch deployment | Needs two real writer PCs and a live network |
| Final cashier/admin UAT | Human desk workflow on real hardware |
| Peripherals (printer, cash drawer) | Phase 2 hardware; not in MVP |
| Local MSVC on a developer PC | Optional for local `tauri build`; **not** required — CI `windows-latest` builds the installer |

Ordinary remaining software work is tracked in CI and in [RC-READINESS.md](RC-READINESS.md), not here.
