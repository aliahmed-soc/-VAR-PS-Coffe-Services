# PlayStation Café POS

Local-first Windows POS and station timer for a two-branch PlayStation café.

## Stack

Tauri 2, React, TypeScript, Tailwind, Rust, SQLx, SQLite, Supabase (Postgres + Auth + RLS + RPC).

React is UI only. Rust owns SQLite, money, pricing, payments, outbox, and all Supabase HTTP.

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

## License

MIT
