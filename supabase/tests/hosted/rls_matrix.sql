-- Hosted RLS matrix. Disposable HT1/HT2 identities only.
-- Requires auth_insert_helper.sql first. Do not add to scripts/run-pg-tests.sh.

CREATE OR REPLACE FUNCTION public.test_rls_expect_deny(p_sql text, p_why text)
RETURNS void
LANGUAGE plpgsql
AS $$
DECLARE
  v_n int;
BEGIN
  BEGIN
    EXECUTE p_sql;
    GET DIAGNOSTICS v_n = ROW_COUNT;
    IF v_n = 0 AND (
      upper(btrim(p_sql)) LIKE 'UPDATE%' OR upper(btrim(p_sql)) LIKE 'DELETE%'
    ) THEN
      RETURN;
    END IF;
    RAISE EXCEPTION 'expected deny: %', p_why;
  EXCEPTION
    WHEN insufficient_privilege THEN
      NULL;
    WHEN others THEN
      IF SQLERRM LIKE 'expected deny:%' THEN
        RAISE;
      END IF;
      RAISE EXCEPTION '% failed with %', p_why, SQLERRM;
  END;
END;
$$;

CREATE OR REPLACE FUNCTION public.hosted_cleanup_rls_matrix()
RETURNS void
LANGUAGE plpgsql
AS $$
DECLARE
  v_b1 uuid := 'aa010001-0000-4000-8000-0000000000b1';
  v_b2 uuid := 'aa010002-0000-4000-8000-0000000000b2';
  v_admin uuid := 'aa0100a1-0000-4000-8000-0000000000a1';
  v_c1 uuid := 'aa0100c1-0000-4000-8000-0000000000c1';
  v_c2 uuid := 'aa0100c2-0000-4000-8000-0000000000c2';
  v_inactive uuid := 'aa0100c9-0000-4000-8000-0000000000c9';
  v_prod uuid := 'aa0100f2-0000-4000-8000-0000000000f2';
  v_cat uuid := 'aa0100f1-0000-4000-8000-0000000000f1';
BEGIN
  RESET ROLE;
  PERFORM public.hosted_set_jwt(NULL);

  DELETE FROM audit_logs WHERE branch_id IN (v_b1, v_b2);
  DELETE FROM sync_receipts WHERE branch_id IN (v_b1, v_b2);
  DELETE FROM device_sequence_cloud WHERE device_id IN (
    'aa0100d1-0000-4000-8000-0000000000d1',
    'aa0100d2-0000-4000-8000-0000000000d2'
  );
  DELETE FROM inventory_movements WHERE branch_id IN (v_b1, v_b2);
  DELETE FROM payments WHERE branch_id IN (v_b1, v_b2);
  DELETE FROM order_items WHERE branch_id IN (v_b1, v_b2);
  DELETE FROM gaming_sessions WHERE branch_id IN (v_b1, v_b2);
  DELETE FROM orders WHERE branch_id IN (v_b1, v_b2);
  DELETE FROM inventory_balances WHERE branch_id IN (v_b1, v_b2);
  DELETE FROM branch_products WHERE branch_id IN (v_b1, v_b2);
  DELETE FROM cashier_shifts WHERE branch_id IN (v_b1, v_b2);
  DELETE FROM expenses WHERE branch_id IN (v_b1, v_b2);
  DELETE FROM pricing_rules WHERE branch_id IN (v_b1, v_b2);
  DELETE FROM stations WHERE branch_id IN (v_b1, v_b2);
  DELETE FROM devices WHERE branch_id IN (v_b1, v_b2);
  DELETE FROM app_settings WHERE id IN (
    'aa010171-0000-4000-8000-000000000171',
    'aa010172-0000-4000-8000-000000000172'
  );
  DELETE FROM user_branch_roles WHERE branch_id IN (v_b1, v_b2);
  DELETE FROM products WHERE id = v_prod;
  DELETE FROM categories WHERE id = v_cat OR name = 'HT Admin Catalog';
  DELETE FROM user_profiles WHERE user_id IN (v_admin, v_c1, v_c2, v_inactive);
  DELETE FROM branches WHERE id IN (v_b1, v_b2) AND code IN ('HT1', 'HT2');
  PERFORM public.hosted_delete_auth_user(v_admin);
  PERFORM public.hosted_delete_auth_user(v_c1);
  PERFORM public.hosted_delete_auth_user(v_c2);
  PERFORM public.hosted_delete_auth_user(v_inactive);
END;
$$;

CREATE OR REPLACE FUNCTION public.test_hosted_rls_matrix()
RETURNS void
LANGUAGE plpgsql
AS $$
DECLARE
  v_b1 uuid := 'aa010001-0000-4000-8000-0000000000b1';
  v_b2 uuid := 'aa010002-0000-4000-8000-0000000000b2';
  v_admin uuid := 'aa0100a1-0000-4000-8000-0000000000a1';
  v_c1 uuid := 'aa0100c1-0000-4000-8000-0000000000c1';
  v_c2 uuid := 'aa0100c2-0000-4000-8000-0000000000c2';
  v_inactive uuid := 'aa0100c9-0000-4000-8000-0000000000c9';
  v_d1 uuid := 'aa0100d1-0000-4000-8000-0000000000d1';
  v_d2 uuid := 'aa0100d2-0000-4000-8000-0000000000d2';
  v_s1 uuid := 'aa0100e1-0000-4000-8000-0000000000e1';
  v_s2 uuid := 'aa0100e2-0000-4000-8000-0000000000e2';
  v_cat uuid := 'aa0100f1-0000-4000-8000-0000000000f1';
  v_prod uuid := 'aa0100f2-0000-4000-8000-0000000000f2';
  v_o1 uuid := 'aa010101-0000-4000-8000-000000000101';
  v_o2 uuid := 'aa010102-0000-4000-8000-000000000102';
  v_og1 uuid := 'aa010111-0000-4000-8000-000000000111';
  v_og2 uuid := 'aa010112-0000-4000-8000-000000000112';
  v_item1 uuid := 'aa010121-0000-4000-8000-000000000121';
  v_item2 uuid := 'aa010122-0000-4000-8000-000000000122';
  v_pay1 uuid := 'aa010131-0000-4000-8000-000000000131';
  v_pay2 uuid := 'aa010132-0000-4000-8000-000000000132';
  v_sess1 uuid := 'aa010141-0000-4000-8000-000000000141';
  v_sess2 uuid := 'aa010142-0000-4000-8000-000000000142';
  v_exp1 uuid := 'aa010151-0000-4000-8000-000000000151';
  v_exp2 uuid := 'aa010152-0000-4000-8000-000000000152';
  v_shift1 uuid := 'aa010161-0000-4000-8000-000000000161';
  v_shift2 uuid := 'aa010162-0000-4000-8000-000000000162';
  v_set_b2 uuid := 'aa010171-0000-4000-8000-000000000171';
  v_set_global uuid := 'aa010172-0000-4000-8000-000000000172';
  v_new_order uuid := 'aa010181-0000-4000-8000-000000000181';
  v_seen int;
  v_qty int;
  v_result jsonb;
BEGIN
  PERFORM public.hosted_cleanup_rls_matrix();

  PERFORM public.hosted_insert_auth_user(v_admin, 'ht-admin@hosted-test.invalid');
  PERFORM public.hosted_insert_auth_user(v_c1, 'ht-c1@hosted-test.invalid');
  PERFORM public.hosted_insert_auth_user(v_c2, 'ht-c2@hosted-test.invalid');
  PERFORM public.hosted_insert_auth_user(v_inactive, 'ht-inactive@hosted-test.invalid');

  INSERT INTO branches (id, code, name) VALUES
    (v_b1, 'HT1', 'Hosted test 1'),
    (v_b2, 'HT2', 'Hosted test 2');
  INSERT INTO user_profiles (user_id, display_name, is_system_admin, is_active) VALUES
    (v_admin, 'HT Admin', true, true),
    (v_c1, 'HT Cashier 1', false, true),
    (v_c2, 'HT Cashier 2', false, true),
    (v_inactive, 'HT Inactive', false, true);
  INSERT INTO user_branch_roles (user_id, branch_id, role, is_active) VALUES
    (v_admin, v_b1, 'admin', true),
    (v_c1, v_b1, 'cashier', true),
    (v_c2, v_b2, 'cashier', true),
    (v_inactive, v_b1, 'cashier', false);
  INSERT INTO devices (id, branch_id, name, device_key, is_active) VALUES
    (v_d1, v_b1, 'HT Cashier HT1', 'ht-dev-ht1', true),
    (v_d2, v_b2, 'HT Cashier HT2', 'ht-dev-ht2', true);
  INSERT INTO stations (id, branch_id, code, display_name) VALUES
    (v_s1, v_b1, 'HTS1', 'HT PS1'),
    (v_s2, v_b2, 'HTS1', 'HT PS1');
  INSERT INTO categories (id, name) VALUES (v_cat, 'HT Drinks');
  INSERT INTO products (id, category_id, sku, name, default_sell_price_minor, default_cost_price_minor)
  VALUES (v_prod, v_cat, 'HT-COKE', 'HT Coke', 1500, 700);
  INSERT INTO branch_products (branch_id, product_id, updated_at) VALUES
    (v_b1, v_prod, now()),
    (v_b2, v_prod, now());
  INSERT INTO inventory_balances (branch_id, product_id, quantity_on_hand, version, updated_at) VALUES
    (v_b1, v_prod, 10, 0, now()),
    (v_b2, v_prod, 99, 0, now());
  INSERT INTO orders (
    id, branch_id, order_type, status, currency_code, opened_by, opened_at
  ) VALUES
    (v_o1, v_b1, 'pos', 'open', 'EGP', v_c1, now()),
    (v_o2, v_b2, 'pos', 'open', 'EGP', v_c2, now()),
    (v_og1, v_b1, 'gaming', 'open', 'EGP', v_c1, now()),
    (v_og2, v_b2, 'gaming', 'open', 'EGP', v_c2, now());
  INSERT INTO order_items (
    id, branch_id, order_id, product_id, product_name_snapshot, quantity,
    unit_price_minor, unit_cost_minor, line_total_minor, added_by, added_at
  ) VALUES
    (v_item1, v_b1, v_o1, v_prod, 'HT Coke', 1, 1500, 700, 1500, v_c1, now()),
    (v_item2, v_b2, v_o2, v_prod, 'HT Coke', 1, 1500, 700, 1500, v_c2, now());
  INSERT INTO payments (
    id, branch_id, order_id, payment_method_id, payment_type,
    amount_due_minor, amount_tendered_minor, amount_applied_minor, change_minor,
    status, cashier_id, paid_at, origin_event_id
  ) VALUES
    (v_pay1, v_b1, v_o1, '11111111-1111-1111-1111-111111111111', 'sale',
     1500, 1500, 1500, 0, 'captured', v_c1, now(), gen_random_uuid()),
    (v_pay2, v_b2, v_o2, '11111111-1111-1111-1111-111111111111', 'sale',
     1500, 1500, 1500, 0, 'captured', v_c2, now(), gen_random_uuid());
  INSERT INTO gaming_sessions (
    id, branch_id, station_id, order_id, status, started_at, pricing_snapshot, started_by
  ) VALUES
    (v_sess1, v_b1, v_s1, v_og1, 'active', now(), '{}'::jsonb, v_c1),
    (v_sess2, v_b2, v_s2, v_og2, 'active', now(), '{}'::jsonb, v_c2);
  INSERT INTO expenses (id, branch_id, category, amount_minor, expense_at, created_by) VALUES
    (v_exp1, v_b1, 'supplies', 100, now(), v_admin),
    (v_exp2, v_b2, 'supplies', 200, now(), v_admin);
  INSERT INTO cashier_shifts (
    id, branch_id, user_id, device_id, status, opening_cash_minor, started_at
  ) VALUES
    (v_shift1, v_b1, v_c1, v_d1, 'open', 0, now()),
    (v_shift2, v_b2, v_c2, v_d2, 'open', 0, now());
  INSERT INTO app_settings (id, scope, branch_id, key, value) VALUES
    (v_set_global, 'global', NULL, 'ht_locale', '{"default":"ar"}'::jsonb),
    (v_set_b2, 'branch', v_b2, 'ht_receipt_prefix', '{"v":"HT2"}'::jsonb);
  INSERT INTO inventory_movements (
    id, branch_id, product_id, movement_type, quantity_delta, quantity_after,
    origin_event_id, created_by, created_at
  ) VALUES
    (gen_random_uuid(), v_b1, v_prod, 'opening', 10, 10, gen_random_uuid(), v_admin, now()),
    (gen_random_uuid(), v_b2, v_prod, 'opening', 99, 99, gen_random_uuid(), v_admin, now());
  INSERT INTO sync_receipts (event_id, branch_id, device_id, local_sequence, event_type, payload_hash)
  VALUES
    (gen_random_uuid(), v_b1, v_d1, 90, 'order.opened', 'hash-ht1'),
    (gen_random_uuid(), v_b2, v_d2, 90, 'order.opened', 'hash-ht2');

  -- Cashier HT1: own branch visible, HT2 hidden, catalog readable.
  PERFORM public.hosted_set_jwt(v_c1);
  SET LOCAL ROLE authenticated;

  SELECT COUNT(*) INTO v_seen FROM orders WHERE branch_id = v_b2;
  IF v_seen <> 0 THEN
    RAISE EXCEPTION 'cashier HT1 must not see HT2 orders';
  END IF;
  SELECT COUNT(*) INTO v_seen FROM orders WHERE branch_id = v_b1;
  IF v_seen < 1 THEN
    RAISE EXCEPTION 'cashier HT1 must see HT1 orders';
  END IF;
  SELECT COUNT(*) INTO v_seen FROM payments WHERE branch_id = v_b2;
  IF v_seen <> 0 THEN
    RAISE EXCEPTION 'cashier HT1 must not see HT2 payments';
  END IF;
  SELECT COUNT(*) INTO v_seen FROM payments WHERE branch_id = v_b1;
  IF v_seen < 1 THEN
    RAISE EXCEPTION 'cashier HT1 must see HT1 payments';
  END IF;
  SELECT COUNT(*) INTO v_seen FROM gaming_sessions WHERE branch_id = v_b2;
  IF v_seen <> 0 THEN
    RAISE EXCEPTION 'cashier HT1 must not see HT2 sessions';
  END IF;
  SELECT quantity_on_hand INTO v_qty FROM inventory_balances WHERE branch_id = v_b2 AND product_id = v_prod;
  IF v_qty IS NOT NULL THEN
    RAISE EXCEPTION 'cashier HT1 must not see HT2 inventory';
  END IF;
  SELECT quantity_on_hand INTO v_qty FROM inventory_balances WHERE branch_id = v_b1 AND product_id = v_prod;
  IF v_qty <> 10 THEN
    RAISE EXCEPTION 'cashier HT1 must see HT1 inventory';
  END IF;
  SELECT COUNT(*) INTO v_seen FROM products WHERE id = v_prod;
  IF v_seen <> 1 THEN
    RAISE EXCEPTION 'cashier must read the product catalog';
  END IF;
  SELECT COUNT(*) INTO v_seen FROM user_profiles WHERE user_id = v_admin;
  IF v_seen <> 0 THEN
    RAISE EXCEPTION 'cashier must not read another profile';
  END IF;
  SELECT COUNT(*) INTO v_seen FROM device_sequence_cloud;
  IF v_seen <> 0 THEN
    RAISE EXCEPTION 'cashier must not read device_sequence_cloud';
  END IF;

  PERFORM public.test_rls_expect_deny(
    format('INSERT INTO orders (id, branch_id, order_type, status, currency_code, opened_by, opened_at) VALUES (%L, %L, %L, %L, %L, %L, now())',
      gen_random_uuid(), v_b1, 'pos', 'open', 'EGP', v_c1),
    'cashier direct order insert'
  );
  PERFORM public.test_rls_expect_deny(
    format('INSERT INTO payments (id, branch_id, order_id, payment_method_id, payment_type, amount_due_minor, amount_tendered_minor, amount_applied_minor, change_minor, status, cashier_id, paid_at, origin_event_id) VALUES (%L, %L, %L, %L, %L, 1, 1, 1, 0, %L, %L, now(), %L)',
      gen_random_uuid(), v_b1, v_o1, '11111111-1111-1111-1111-111111111111', 'sale', 'captured', v_c1, gen_random_uuid()),
    'cashier direct payment insert'
  );

  BEGIN
    PERFORM public.apply_domain_event(
      gen_random_uuid(), v_b2, v_d2, 1, 'order.opened',
      jsonb_build_object('order_id', gen_random_uuid(), 'order_type', 'pos', 'opened_by', v_c1, 'opened_at', now()),
      'hash-forbidden'
    );
    RAISE EXCEPTION 'cashier HT1 apply on HT2 must fail';
  EXCEPTION
    WHEN insufficient_privilege THEN
      NULL;
    WHEN others THEN
      IF SQLERRM LIKE '%cashier HT1 apply on HT2 must fail%' THEN
        RAISE;
      END IF;
      IF SQLERRM NOT LIKE '%branch_forbidden%' THEN
        RAISE;
      END IF;
  END;

  SELECT public.apply_domain_event(
    gen_random_uuid(), v_b1, v_d1, 1, 'order.opened',
    jsonb_build_object(
      'order_id', v_new_order,
      'order_type', 'pos',
      'opened_by', v_c1,
      'opened_at', now()
    ),
    'hash-ht-open'
  ) INTO v_result;
  IF v_result->>'status' <> 'applied' THEN
    RAISE EXCEPTION 'cashier HT1 SECURITY DEFINER apply on own branch must work';
  END IF;
  RESET ROLE;

  -- Cashier HT2 cannot see HT1.
  PERFORM public.hosted_set_jwt(v_c2);
  SET LOCAL ROLE authenticated;
  SELECT COUNT(*) INTO v_seen FROM orders WHERE branch_id = v_b1;
  IF v_seen <> 0 THEN
    RAISE EXCEPTION 'cashier HT2 must not see HT1 orders';
  END IF;
  SELECT COUNT(*) INTO v_seen FROM orders WHERE branch_id = v_b2;
  IF v_seen < 1 THEN
    RAISE EXCEPTION 'cashier HT2 must see HT2 orders';
  END IF;
  RESET ROLE;

  -- Inactive HT1 cashier is denied the branch.
  PERFORM public.hosted_set_jwt(v_inactive);
  SET LOCAL ROLE authenticated;
  SELECT COUNT(*) INTO v_seen FROM orders WHERE branch_id = v_b1;
  IF v_seen <> 0 THEN
    RAISE EXCEPTION 'inactive cashier must not see HT1 orders';
  END IF;
  RESET ROLE;

  -- System admin sees both branches and may write catalog, not money tables.
  PERFORM public.hosted_set_jwt(v_admin);
  SET LOCAL ROLE authenticated;
  SELECT COUNT(*) INTO v_seen FROM orders WHERE branch_id IN (v_b1, v_b2);
  IF v_seen < 2 THEN
    RAISE EXCEPTION 'admin must see both branches';
  END IF;
  SELECT COUNT(*) INTO v_seen FROM payments WHERE branch_id IN (v_b1, v_b2);
  IF v_seen < 2 THEN
    RAISE EXCEPTION 'admin must see both payments';
  END IF;
  INSERT INTO categories (name, sort_order) VALUES ('HT Admin Catalog', 99);
  PERFORM public.test_rls_expect_deny(
    format('INSERT INTO orders (id, branch_id, order_type, status, currency_code, opened_by, opened_at) VALUES (%L, %L, %L, %L, %L, %L, now())',
      gen_random_uuid(), v_b1, 'pos', 'open', 'EGP', v_admin),
    'admin direct order insert'
  );
  PERFORM public.test_rls_expect_deny(
    format('UPDATE payments SET amount_applied_minor = 1 WHERE id = %L', v_pay1),
    'admin direct payment update'
  );
  RESET ROLE;

  -- Anonymous has no table grants and no JWT.
  PERFORM public.hosted_set_jwt(NULL);
  SET LOCAL ROLE anon;
  PERFORM public.test_rls_expect_deny('SELECT COUNT(*) FROM orders', 'anon orders');
  PERFORM public.test_rls_expect_deny('SELECT COUNT(*) FROM payments', 'anon payments');
  PERFORM public.test_rls_expect_deny('SELECT COUNT(*) FROM inventory_balances', 'anon inventory');
  -- Hosted default GRANT + RLS: SELECT may succeed with 0 rows instead of 42501.
  BEGIN
    SELECT COUNT(*) INTO v_seen FROM user_profiles;
    IF v_seen <> 0 THEN
      RAISE EXCEPTION 'anon must not see user_profiles';
    END IF;
  EXCEPTION
    WHEN insufficient_privilege THEN
      NULL;
  END;
  BEGIN
    PERFORM public.apply_domain_event(
      gen_random_uuid(), v_b1, v_d1, 2, 'order.opened',
      jsonb_build_object('order_id', gen_random_uuid(), 'order_type', 'pos', 'opened_by', v_c1, 'opened_at', now()),
      'hash-anon'
    );
    RAISE EXCEPTION 'anon apply_domain_event must fail';
  EXCEPTION
    WHEN others THEN
      IF SQLERRM LIKE '%anon apply_domain_event must fail%' THEN
        RAISE;
      END IF;
      IF SQLERRM NOT LIKE '%not_authenticated%' AND SQLSTATE <> '42501' THEN
        RAISE;
      END IF;
  END;
  RESET ROLE;
  PERFORM public.hosted_set_jwt(NULL);

  PERFORM public.hosted_cleanup_rls_matrix();
EXCEPTION
  WHEN others THEN
    BEGIN
      PERFORM public.hosted_cleanup_rls_matrix();
    EXCEPTION
      WHEN others THEN
        NULL;
    END;
    RAISE;
END;
$$;

SELECT public.test_hosted_rls_matrix();
