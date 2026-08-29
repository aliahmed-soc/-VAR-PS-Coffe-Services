# PlayStation Café POS

Local-first Windows POS and station timer for a two-branch PlayStation café.

Repository: https://github.com/aliahmed-soc/-VAR-PS-Coffe-Services

## Stack

Tauri 2, React, TypeScript, Tailwind, Rust, SQLx, SQLite, Supabase (Postgres + Auth + RLS + RPC).

React is UI only. Rust owns SQLite, money, pricing, payments, outbox, and all Supabase HTTP. There is no `supabase-js` in the webview.

Canonical architecture: [`docs/architecture/ARCHITECTURE.md`](docs/architecture/ARCHITECTURE.md).

## Development

1. Install Rust stable (MSVC) and Visual Studio 2022 Build Tools with the C++ workload.
2. Copy `.env.example` to `.env` for local Supabase (optional for offline demo).
3. `npm install`
4. `npx supabase start` when you need cloud sync (local CLI only).
5. `npm run tauri dev`

Demo data: use **Load demo data**, then offline unlock as `u-c1` / PIN `1357`.

## Tests

```
npm test
cargo test --manifest-path src-tauri/Cargo.toml
```

CI on `main`: contracts, cafe-domain, postgres, tauri-windows.

## Windows installer

Do not require a local Visual Studio C++ install. Dispatch **Package Windows NSIS** on GitHub Actions (`workflow_dispatch`). That job builds an unsigned NSIS installer, validates it, records SHA-256, and uploads the artifact. It does not publish a GitHub Release.

Details: [`docs/release/windows-packaging.md`](docs/release/windows-packaging.md). Readiness: [`docs/release/RC-READINESS.md`](docs/release/RC-READINESS.md).

## License

MIT
