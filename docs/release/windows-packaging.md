# Windows NSIS packaging

The installer is built on GitHub Actions `windows-latest` (MSVC present). A local Visual Studio C++ install is **not** required.

## Workflow

- File: `.github/workflows/package-windows.yml`
- Trigger: `workflow_dispatch` only (optional version-tag trigger can be added later)
- Does **not** publish a GitHub Release
- Produces an **unsigned** NSIS installer and uploads it as a workflow artifact

Dispatch:

```
gh workflow run "Package Windows NSIS" --ref <green-sha>
```

Or use the Actions UI. Prefer a commit that already has the four required CI jobs green.

## What CI proves

1. Checkout of the exact requested commit
2. Node 22 + `npm ci`
3. Stable Rust MSVC
4. Frontend production build
5. Contract tests + `cafe-domain` tests
6. `npm run tauri -- build --bundles nsis --config src-tauri/tauri.release.conf.json`
7. Release binary + NSIS `.exe` exist and are non-empty
8. Installer name contains `package.json` version
9. Bundle directory contains no `.env`, SQLite, PEM, or credential files
10. SHA-256 recorded in `nsis-sha256.txt` (artifact, not git)
11. Isolated silent install: no shipped database; upgrade does not overwrite an adjacent SQLite marker; uninstall does not wipe that marker

## Artifact location

GitHub Actions → Package Windows NSIS → Artifacts → `playstation-cafe-nsis-unsigned-<shortsha>`

Expected file name: `PlayStation Cafe POS_<version>_x64-setup.exe`

Build trees stay in `src-tauri/target` (gitignored).

## Signing

Building and signing are separate. Missing Authenticode material must not fail the package job. See [signing.md](signing.md).

This unsigned installer is **development / pre-release**. It is not production-distributable until signed.

## Runtime storage (survives upgrades)

Created on first launch, not shipped in the installer:

| Item | Location |
| --- | --- |
| SQLite database | `{app_data_dir}/branch.sqlite` |
| Device id | `{app_data_dir}/device_id` |
| Backups | `{app_data_dir}/backups/branch-<UTC>.sqlite` (retain 14) |
| Staged restore | `{app_data_dir}/branch.sqlite.restore` |

On Windows, `app_data_dir` is the Tauri identifier directory (`com.playstationcafe.pos`) under the user AppData tree.

Uninstall does **not** delete business data. That is intentional.

## Release config overlay

`src-tauri/tauri.release.conf.json` drops Vite `localhost:1420/1421` from CSP. Dev `tauri.conf.json` keeps those connect-src entries for `tauri dev` only.

Release builds refuse loopback Supabase URLs. Debug builds refuse hosted `*.supabase.co` unless `PSC_ALLOW_PROD=1`.
