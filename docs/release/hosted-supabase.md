# Hosted production Supabase

Project ref: `rbxtxtlssknjioaveytg`  
API URL: `https://rbxtxtlssknjioaveytg.supabase.co`

The desktop app uses only this URL plus a **publishable** key (`sb_publishable_...`) and the signed-in user's JWT. Rust is the only cloud client.

## Forbidden

- `sb_secret_...` (treat any previously exposed secret as compromised; rotate it)
- Legacy `service_role` / JWT admin keys
- JWT signing private keys / legacy HS256 secret
- `supabase/seed/dev.sql` against production
- `supabase db reset` against the hosted project

## Desktop configuration

Runtime (preferred on a cashier PC) or compile-time CI secret:

| Name | Value |
| --- | --- |
| `PSC_SUPABASE_URL` or `SUPABASE_URL` | `https://rbxtxtlssknjioaveytg.supabase.co` |
| `PSC_SUPABASE_ANON_KEY` or `SUPABASE_PUBLISHABLE_KEY` | `sb_publishable_...` only |

Release builds default the URL to the hosted project when unset. The publishable key is never committed. Debug builds still refuse hosted `*.supabase.co` unless `PSC_ALLOW_PROD=1`.

## Migration deploy (operator)

The publishable key cannot apply DDL. Use the GitHub Actions workflow **Hosted Supabase migrate** (`workflow_dispatch` only) or `pwsh -File scripts/hosted/push-migrations.ps1`.

Both paths require these repository / process secrets (names only — never paste values into git or docs):

- `SUPABASE_ACCESS_TOKEN` — Supabase CLI personal access token
- `SUPABASE_DB_PASSWORD` — hosted Postgres database password

Neither secret may be a project API key (`sb_secret_...`, `sb_publishable_...`, or `service_role`).

Sequence (non-interactive, never `db reset`, never `--include-seed`):

1. Link project `rbxtxtlssknjioaveytg`
2. Show remote migration history
3. Dry-run `db push` when the CLI supports `--dry-run` (skip if not)
4. Fail if public application tables exist with zero matching repo versions, or unexpected conflicting tables
5. Apply only outstanding repo migrations: `20260829000100_init`, `20260829000200_rls`, `20260829000300_apply_domain_event`
6. Run `supabase/tests/hosted/validate_schema.sql`
7. Run hosted RLS matrix and event acceptance (disposable `HT1` / `HT2` / `HTE` only)

Alternatively, apply the three SQL files in order with `psql` / `supabase db push --db-url` using `SUPABASE_DB_PASSWORD`. Do not invent a CLI PAT. Do not use the desktop publishable key as a migration credential.

IPv4 session pooler for this project (not a credential): host `aws-1-eu-west-1.pooler.supabase.com`, port `5432`, user `postgres.rbxtxtlssknjioaveytg`, `sslmode=require`. Direct `db.<ref>.supabase.co:5432` is IPv6-only.

## Production bootstrap

After schema validation: create Auth users in the dashboard, then fill and apply `supabase/bootstrap/production.sql.template`. Operator supplies real emails, display names, station codes, device keys, and linear hourly rates. Do not invent live product inventory here.

## GitHub Actions

Repository secrets (never `sb_secret_`):

- `PSC_SUPABASE_ANON_KEY` — publishable only; used to compile a production-pointed RC
- `SUPABASE_ACCESS_TOKEN` — CLI token for the hosted push workflow
- `SUPABASE_DB_PASSWORD` — hosted Postgres password for the hosted push workflow
