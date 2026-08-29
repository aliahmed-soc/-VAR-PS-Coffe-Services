# Windows code signing (external)

No certificate exists in this repository and none should be committed.

## Policy

- CI **builds** an unsigned NSIS installer even when no signing secrets are present.
- Unsigned builds are labeled development / pre-release.
- Do not embed fake certificates, self-signed “production” certs, or placeholder secrets.
- Do not call the application production-distributable until Authenticode signing is in place.

## Later: GitHub Actions secrets

When a real certificate is available, add repository secrets (names reserved; job not enabled yet):

| Secret | Purpose |
| --- | --- |
| `WINDOWS_CERT_P12` | Base64-encoded `.p12` / `.pfx` (never commit the file) |
| `WINDOWS_CERT_PASSWORD` | Certificate password |
| `TAURI_SIGNING_PRIVATE_KEY` | Optional Tauri updater key (separate from Authenticode) |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Optional updater key password |

The package workflow already leaves `TAURI_SIGNING_PRIVATE_KEY` and `WINDOWS_CERTIFICATE_THUMBPRINT` unset so the unsigned path is explicit.

A future signing step should consume the unsigned artifact, sign it, and only then attach it to a GitHub Release. Do not auto-publish from the current workflow.
