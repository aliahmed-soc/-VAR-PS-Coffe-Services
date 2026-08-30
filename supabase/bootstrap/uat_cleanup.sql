-- Remove disposable UAT rows created for hosted acceptance.
-- DO NOT RUN until physical UAT is complete.
-- Does not drop migrations, payment_methods.cash, or non-UAT data.

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
  SELECT id FROM public.devices WHERE branch_id IN (
    SELECT id FROM public.branches WHERE code IN ('UAT1', 'UAT2')
  )
);

DELETE FROM public.audit_logs
WHERE branch_id IN (SELECT id FROM public.branches WHERE code IN ('UAT1', 'UAT2'));

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
WHERE branch_id IN (SELECT id FROM public.branches WHERE code IN ('UAT1', 'UAT2'));

DELETE FROM public.products
WHERE sku LIKE 'UAT-%';

DELETE FROM public.categories
WHERE name = 'UAT Category';

DELETE FROM public.user_profiles
WHERE user_id IN (
  SELECT id FROM auth.users WHERE email LIKE '%@invalid.test'
);

DELETE FROM public.branches
WHERE code IN ('UAT1', 'UAT2');

DELETE FROM auth.identities
WHERE user_id IN (SELECT id FROM auth.users WHERE email LIKE '%@invalid.test');

DELETE FROM auth.users
WHERE email LIKE '%@invalid.test';

COMMIT;
