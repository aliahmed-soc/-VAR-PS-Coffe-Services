-- Hosted schema/RPC/constraint acceptance. Read-only except for privilege probes.
-- Hosted-only: do not add this file to scripts/run-pg-tests.sh.

DO $$
DECLARE
  missing text := '';
  t text;
  expected text[] := ARRAY[
    'branches','user_profiles','user_branch_roles','devices','stations','pricing_rules',
    'categories','products','branch_products','inventory_balances','inventory_movements',
    'orders','order_items','gaming_sessions','payment_methods','payments','expenses',
    'cashier_shifts','audit_logs','app_settings','sync_receipts','device_sequence_cloud'
  ];
  money text[] := ARRAY[
    'payments','orders','order_items','inventory_balances','inventory_movements',
    'gaming_sessions','sync_receipts'
  ];
  rls_off text := '';
  versions int := 0;
  grant_bad text := '';
BEGIN
  FOREACH t IN ARRAY expected LOOP
    IF NOT EXISTS (
      SELECT 1 FROM information_schema.tables
      WHERE table_schema = 'public' AND table_name = t
    ) THEN
      missing := missing || t || ',';
    END IF;
    IF EXISTS (
      SELECT 1 FROM pg_class c
      JOIN pg_namespace n ON n.oid = c.relnamespace
      WHERE n.nspname = 'public' AND c.relname = t AND c.relkind = 'r' AND NOT c.relrowsecurity
    ) THEN
      rls_off := rls_off || t || ',';
    END IF;
  END LOOP;
  IF missing <> '' THEN
    RAISE EXCEPTION 'missing tables: %', missing;
  END IF;
  IF rls_off <> '' THEN
    RAISE EXCEPTION 'RLS disabled: %', rls_off;
  END IF;

  IF to_regclass('supabase_migrations.schema_migrations') IS NULL THEN
    RAISE EXCEPTION 'supabase_migrations.schema_migrations missing';
  END IF;
  EXECUTE $v$
    SELECT COUNT(DISTINCT left(version::text, 14))
    FROM supabase_migrations.schema_migrations
    WHERE version::text LIKE '20260829000100%'
       OR version::text LIKE '20260829000200%'
       OR version::text LIKE '20260829000300%'
  $v$ INTO versions;
  IF versions <> 3 THEN
    RAISE EXCEPTION 'expected three repo migration versions, found %', versions;
  END IF;

  IF NOT EXISTS (
    SELECT 1 FROM pg_proc p
    JOIN pg_namespace n ON n.oid = p.pronamespace
    WHERE n.nspname = 'public' AND p.proname = 'apply_domain_event'
  ) THEN
    RAISE EXCEPTION 'apply_domain_event missing';
  END IF;
  IF NOT EXISTS (
    SELECT 1 FROM pg_proc p
    JOIN pg_namespace n ON n.oid = p.pronamespace
    WHERE n.nspname = 'public' AND p.proname = 'pull_branch_since'
  ) THEN
    RAISE EXCEPTION 'pull_branch_since missing';
  END IF;

  IF NOT EXISTS (
    SELECT 1 FROM pg_indexes WHERE schemaname = 'public' AND indexname = 'idx_payments_one_captured_sale'
  ) THEN
    RAISE EXCEPTION 'one captured sale index missing';
  END IF;
  IF NOT EXISTS (
    SELECT 1 FROM pg_indexes WHERE schemaname = 'public' AND indexname = 'idx_gaming_one_active_station'
  ) THEN
    RAISE EXCEPTION 'one active session index missing';
  END IF;
  IF NOT EXISTS (
    SELECT 1 FROM pg_indexes WHERE schemaname = 'public' AND indexname = 'idx_sync_receipts_device_seq'
  ) THEN
    RAISE EXCEPTION 'device sequence uniqueness index missing';
  END IF;
  IF NOT EXISTS (
    SELECT 1
    FROM pg_constraint c
    JOIN pg_class t2 ON t2.oid = c.conrelid
    JOIN pg_namespace n ON n.oid = t2.relnamespace
    WHERE n.nspname = 'public' AND t2.relname = 'sync_receipts' AND c.contype = 'p'
  ) THEN
    RAISE EXCEPTION 'sync_receipts event_id primary key missing';
  END IF;
  IF NOT EXISTS (
    SELECT 1
    FROM pg_constraint c
    JOIN pg_class t2 ON t2.oid = c.conrelid
    JOIN pg_namespace n ON n.oid = t2.relnamespace
    WHERE n.nspname = 'public'
      AND t2.relname = 'inventory_movements'
      AND c.contype = 'u'
      AND pg_get_constraintdef(c.oid) ILIKE '%origin_event_id%'
  ) THEN
    RAISE EXCEPTION 'inventory_movements origin_event_id uniqueness missing';
  END IF;
  IF NOT EXISTS (
    SELECT 1
    FROM pg_constraint c
    JOIN pg_class t2 ON t2.oid = c.conrelid
    JOIN pg_namespace n ON n.oid = t2.relnamespace
    WHERE n.nspname = 'public'
      AND t2.relname = 'payments'
      AND c.contype = 'u'
      AND pg_get_constraintdef(c.oid) ILIKE '%origin_event_id%'
  ) THEN
    RAISE EXCEPTION 'payments origin_event_id uniqueness missing';
  END IF;

  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint WHERE conname = 'orders_subtotal_identity'
  ) THEN
    RAISE EXCEPTION 'orders_subtotal_identity missing';
  END IF;
  IF NOT EXISTS (
    SELECT 1 FROM pg_trigger WHERE tgname = 'orders_paid_tax_immutable'
  ) THEN
    RAISE EXCEPTION 'orders_paid_tax_immutable trigger missing';
  END IF;

  IF NOT EXISTS (
    SELECT 1
    FROM pg_constraint c
    JOIN pg_class t2 ON t2.oid = c.conrelid
    JOIN pg_namespace n ON n.oid = t2.relnamespace
    WHERE n.nspname = 'public'
      AND t2.relname = 'pricing_rules'
      AND pg_get_constraintdef(c.oid) ILIKE '%linear%'
      AND pg_get_constraintdef(c.oid) NOT ILIKE '%stepped%'
  ) THEN
    RAISE EXCEPTION 'pricing_rules must accept only linear';
  END IF;

  IF NOT EXISTS (
    SELECT 1
    FROM pg_constraint c
    JOIN pg_class t2 ON t2.oid = c.conrelid
    JOIN pg_namespace n ON n.oid = t2.relnamespace
    WHERE n.nspname = 'public'
      AND t2.relname = 'pricing_rules'
      AND pg_get_constraintdef(c.oid) ILIKE '%rate_minor_per_hour%>=%0%'
  ) THEN
    RAISE EXCEPTION 'pricing_rules must require rate_minor_per_hour >= 0';
  END IF;

  IF NOT EXISTS (
    SELECT 1
    FROM pg_constraint c
    JOIN pg_class t2 ON t2.oid = c.conrelid
    JOIN pg_namespace n ON n.oid = t2.relnamespace
    WHERE n.nspname = 'public'
      AND t2.relname = 'inventory_balances'
      AND pg_get_constraintdef(c.oid) ILIKE '%quantity_on_hand%>=%0%'
  ) THEN
    RAISE EXCEPTION 'inventory_balances must reject negative quantity';
  END IF;

  FOREACH t IN ARRAY money LOOP
    IF NOT has_table_privilege('authenticated', format('public.%I', t), 'SELECT') THEN
      grant_bad := grant_bad || t || ':missing SELECT,';
    END IF;
    IF has_table_privilege('authenticated', format('public.%I', t), 'INSERT') THEN
      grant_bad := grant_bad || t || ':INSERT allowed,';
    END IF;
    IF has_table_privilege('authenticated', format('public.%I', t), 'UPDATE') THEN
      grant_bad := grant_bad || t || ':UPDATE allowed,';
    END IF;
    IF has_table_privilege('authenticated', format('public.%I', t), 'DELETE') THEN
      grant_bad := grant_bad || t || ':DELETE allowed,';
    END IF;
  END LOOP;
  IF grant_bad <> '' THEN
    RAISE EXCEPTION 'authenticated money-table grants must be SELECT-only: %', grant_bad;
  END IF;
END $$;
