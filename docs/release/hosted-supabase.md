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

The publishable key cannot apply DDL. Use one of:

1. Supabase CLI personal access token (`supabase login` or `SUPABASE_ACCESS_TOKEN`), then `pwsh -File scripts/hosted/push-migrations.ps1`
2. Hosted Postgres database password with `psql` against the repository SQL files in order — never `db reset`

Then run `supabase/tests/hosted/validate_schema.sql` on the linked project.

## Production bootstrap

After schema validation: create Auth users in the dashboard, then fill and apply `supabase/bootstrap/production.sql.template`. Operator supplies real emails, display names, station codes, device keys, and linear hourly rates. Do not invent live product inventory here.

## GitHub Actions (optional later)

Repository secrets (never `sb_secret_`):

- `PSC_SUPABASE_ANON_KEY` — publishable only; used to compile a production-pointed RC
- `SUPABASE_ACCESS_TOKEN` — CLI token for the hosted push workflow
