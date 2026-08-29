-- Pre-push conflict check for the hosted project.
-- Fail if application tables exist with zero matching repo migration versions,
-- or if unexpected conflicting public tables are present.
-- Empty public schema is OK. Do not add this file to scripts/run-pg-tests.sh.

DO $$
DECLARE
  expected text[] := ARRAY[
    'branches','user_profiles','user_branch_roles','devices','stations','pricing_rules',
    'categories','products','branch_products','inventory_balances','inventory_movements',
    'orders','order_items','gaming_sessions','payment_methods','payments','expenses',
    'cashier_shifts','audit_logs','app_settings','sync_receipts','device_sequence_cloud'
  ];
  app_found int := 0;
  matching_versions int := 0;
  extra text := '';
  conflict_extra text := '';
  t text;
  version_sql text;
BEGIN
  FOREACH t IN ARRAY expected LOOP
    IF EXISTS (
      SELECT 1 FROM information_schema.tables
      WHERE table_schema = 'public' AND table_type = 'BASE TABLE' AND table_name = t
    ) THEN
      app_found := app_found + 1;
    END IF;
  END LOOP;

  SELECT string_agg(c.relname, ',' ORDER BY c.relname)
    INTO extra
  FROM pg_class c
  JOIN pg_namespace n ON n.oid = c.relnamespace
  WHERE n.nspname = 'public'
    AND c.relkind = 'r'
    AND NOT c.relname = ANY (expected);

  IF extra IS NOT NULL AND extra <> '' THEN
    SELECT string_agg(x, ',' ORDER BY x)
      INTO conflict_extra
    FROM unnest(string_to_array(extra, ',')) AS x
    WHERE x ~* '(^|_)(order|orders|payment|payments|session|sessions|inventor|branch|cashier|product|station|sync_receipt|pricing|gaming)(s|_)?';
  END IF;

  IF to_regclass('supabase_migrations.schema_migrations') IS NOT NULL
     AND EXISTS (
       SELECT 1 FROM information_schema.columns
       WHERE table_schema = 'supabase_migrations'
         AND table_name = 'schema_migrations'
         AND column_name = 'version'
     )
  THEN
    version_sql := $v$
      SELECT COUNT(DISTINCT left(version::text, 14))
      FROM supabase_migrations.schema_migrations
      WHERE version::text LIKE '20260829000100%'
         OR version::text LIKE '20260829000200%'
         OR version::text LIKE '20260829000300%'
    $v$;
    EXECUTE version_sql INTO matching_versions;
  END IF;

  IF conflict_extra IS NOT NULL AND conflict_extra <> '' THEN
    RAISE EXCEPTION 'unexpected conflicting application tables: %', conflict_extra;
  END IF;

  IF app_found > 0 AND matching_versions = 0 THEN
    RAISE EXCEPTION
      'application tables exist (%) with zero matching repo migration versions (20260829000100/200/300)',
      app_found;
  END IF;
END $$;
