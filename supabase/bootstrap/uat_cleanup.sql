-- Remove disposable UAT rows created for hosted acceptance.
-- DO NOT RUN until physical UAT is complete.
-- Does not drop migrations, payment_methods.cash, or non-UAT data.

-- Exact disposable Auth UUIDs (not every *@invalid.test):
--   a11e0001-0a11-4000-a000-000000000001  uat-admin@invalid.test
--   a11e0001-0a11-4000-a000-000000000002  uat-b1-cashier@invalid.test
--   a11e0001-0a11-4000-a000-000000000003  uat-b2-cashier@invalid.test

BEGIN;

DELETE FROM public.inventory_movements
WHERE branch_id IN (SELECT id FROM public.branches WHERE code IN ('UAT1', 'UAT2'));

DELETE FROM public.payments
WHERE branch_id IN (SELECT id FROM public.branches WHERE code IN ('UAT1', 'UAT2'));

DELETE FROM public.order_items
WHERE branch_id IN (SELECT id FROM public.branches WHERE code IN ('UAT1', 'UAT2'));

DELETE FROM public.gaming_sessions
WHERE branch_id IN (SELECT id FROM public.branches WHERE code IN ('UAT1', 'UAT2'));

DELETE FROM public.orders
WHERE branch_id IN (SELECT id FROM public.branches WHERE code IN ('UAT1', 'UAT2'));

DELETE FROM public.sync_receipts
WHERE branch_id IN (SELECT id FROM public.branches WHERE code IN ('UAT1', 'UAT2'));

DELETE FROM public.device_sequence_cloud
WHERE device_id IN (
  SELECT id FROM public.devices
  WHERE branch_id IN (SELECT id FROM public.branches WHERE code IN ('UAT1', 'UAT2'))
);

DELETE FROM public.audit_logs
WHERE branch_id IN (SELECT id FROM public.branches WHERE code IN ('UAT1', 'UAT2'));

DELETE FROM public.cashier_shifts
WHERE branch_id IN (SELECT id FROM public.branches WHERE code IN ('UAT1', 'UAT2'))
   OR device_id IN (
     SELECT id FROM public.devices
     WHERE branch_id IN (SELECT id FROM public.branches WHERE code IN ('UAT1', 'UAT2'))
   );

DELETE FROM public.expenses
WHERE branch_id IN (SELECT id FROM public.branches WHERE code IN ('UAT1', 'UAT2'));

DELETE FROM public.app_settings
WHERE branch_id IN (SELECT id FROM public.branches WHERE code IN ('UAT1', 'UAT2'))
   OR device_id IN (
     SELECT id FROM public.devices
     WHERE branch_id IN (SELECT id FROM public.branches WHERE code IN ('UAT1', 'UAT2'))
   );

DELETE FROM public.inventory_balances
WHERE branch_id IN (SELECT id FROM public.branches WHERE code IN ('UAT1', 'UAT2'));

DELETE FROM public.branch_products
WHERE branch_id IN (SELECT id FROM public.branches WHERE code IN ('UAT1', 'UAT2'));

DELETE FROM public.pricing_rules
WHERE branch_id IN (SELECT id FROM public.branches WHERE code IN ('UAT1', 'UAT2'));

DELETE FROM public.stations
WHERE branch_id IN (SELECT id FROM public.branches WHERE code IN ('UAT1', 'UAT2'));

DELETE FROM public.devices
WHERE branch_id IN (SELECT id FROM public.branches WHERE code IN ('UAT1', 'UAT2'));

DELETE FROM public.user_branch_roles
WHERE branch_id IN (SELECT id FROM public.branches WHERE code IN ('UAT1', 'UAT2'))
   OR user_id IN (
     'a11e0001-0a11-4000-a000-000000000001'::uuid,
     'a11e0001-0a11-4000-a000-000000000002'::uuid,
     'a11e0001-0a11-4000-a000-000000000003'::uuid
   );

DELETE FROM public.products
WHERE sku IN ('UAT-DRINK', 'UAT-SNACK');

DELETE FROM public.categories
WHERE id = 'a11e0001-0a11-4000-c000-000000000001'::uuid
   OR name = 'UAT Category';

DELETE FROM public.user_profiles
WHERE user_id IN (
  'a11e0001-0a11-4000-a000-000000000001'::uuid,
  'a11e0001-0a11-4000-a000-000000000002'::uuid,
  'a11e0001-0a11-4000-a000-000000000003'::uuid
);

DELETE FROM public.branches
WHERE code IN ('UAT1', 'UAT2');

DELETE FROM auth.identities
WHERE user_id IN (
  'a11e0001-0a11-4000-a000-000000000001'::uuid,
  'a11e0001-0a11-4000-a000-000000000002'::uuid,
  'a11e0001-0a11-4000-a000-000000000003'::uuid
);

DELETE FROM auth.users
WHERE id IN (
  'a11e0001-0a11-4000-a000-000000000001'::uuid,
  'a11e0001-0a11-4000-a000-000000000002'::uuid,
  'a11e0001-0a11-4000-a000-000000000003'::uuid
);

COMMIT;
