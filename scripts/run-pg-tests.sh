#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
export PGPASSWORD="${PGPASSWORD:-postgres}"
HOST="${PGHOST:-127.0.0.1}"
PORT="${PGPORT:-5432}"
USER="${PGUSER:-postgres}"
DB="${PGDATABASE:-playstation_cafe_test}"

psql -h "$HOST" -p "$PORT" -U "$USER" -d postgres -v ON_ERROR_STOP=1 -c "DROP DATABASE IF EXISTS ${DB};"
psql -h "$HOST" -p "$PORT" -U "$USER" -d postgres -v ON_ERROR_STOP=1 -c "CREATE DATABASE ${DB};"
psql -h "$HOST" -p "$PORT" -U "$USER" -d "$DB" -v ON_ERROR_STOP=1 -f "$ROOT/supabase/tests/harness/00_auth_stub.sql"
# Every migration, in order, so a later one that redefines an RPC is what the
# contracts below actually run against.
for migration in "$ROOT"/supabase/migrations/*.sql; do
  psql -h "$HOST" -p "$PORT" -U "$USER" -d "$DB" -v ON_ERROR_STOP=1 -f "$migration"
done
psql -h "$HOST" -p "$PORT" -U "$USER" -d "$DB" -v ON_ERROR_STOP=1 -f "$ROOT/supabase/tests/payment/atomicity.sql"
psql -h "$HOST" -p "$PORT" -U "$USER" -d "$DB" -v ON_ERROR_STOP=1 -f "$ROOT/supabase/tests/tax/tax_identity.sql"
psql -h "$HOST" -p "$PORT" -U "$USER" -d "$DB" -v ON_ERROR_STOP=1 -f "$ROOT/supabase/tests/rls/branch_isolation.sql"
psql -h "$HOST" -p "$PORT" -U "$USER" -d "$DB" -v ON_ERROR_STOP=1 -f "$ROOT/supabase/tests/pricing/linear_mvp.sql"
psql -h "$HOST" -p "$PORT" -U "$USER" -d "$DB" -v ON_ERROR_STOP=1 -f "$ROOT/supabase/tests/inventory/order_void.sql"
echo "PostgreSQL contract tests passed"
