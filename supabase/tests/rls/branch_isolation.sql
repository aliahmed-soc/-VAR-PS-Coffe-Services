-- RLS allow/deny matrix for vanilla Postgres CI (auth stub + SET ROLE).

CREATE OR REPLACE FUNCTION public.test_rls_expect_deny(p_sql text, p_why text)
RETURNS void
LANGUAGE plpgsql
AS $$
BEGIN
  BEGIN
    EXECUTE p_sql;
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

CREATE OR REPLACE FUNCTION public.test_rls_branch_isolation()
RETURNS void
LANGUAGE plpgsql
AS $$
DECLARE
  v_b1 uuid := '11111111-1111-4111-8111-111111111111';
  v_b2 uuid := 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa';
  v_admin uuid := '33333333-3333-4333-8333-333333333333';
  v_c1 uuid := 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb';
  v_c2 uuid := '21212121-2121-4212-8212-212121212121';
  v_inactive uuid := '31313131-3131-4313-8313-313131313131';
  v_d1 uuid := '41414141-4141-4414-8414-414141414141';
  v_d2 uuid := '51515151-5151-4515-8515-515151515151';
  v_s1 uuid := '61616161-6161-4616-8616-616161616161';
  v_s2 uuid := '71717171-7171-4717-8717-717171717171';
  v_cat uuid := '81818181-8181-4818-8818-818181818181';
  v_prod uuid := '91919191-9191-4919-8919-919191919191';
  v_o1 uuid := 'a1a1a1a1-a1a1-41a1-81a1-a1a1a1a1a1a1';
  v_o2 uuid := 'a2a2a2a2-a2a2-42a2-82a2-a2a2a2a2a2a2';
  v_og1 uuid := 'a3a3a3a3-a3a3-43a3-83a3-a3a3a3a3a3a3';
  v_og2 uuid := 'a4a4a4a4-a4a4-44a4-84a4-a4a4a4a4a4a4';
  v_item1 uuid := 'b1b1b1b1-b1b1-41b1-81b1-b1b1b1b1b1b1';
  v_item2 uuid := 'b2b2b2b2-b2b2-42b2-82b2-b2b2b2b2b2b2';
  v_pay1 uuid := 'c1c1c1c1-c1c1-41c1-81c1-c1c1c1c1c1c1';
  v_pay2 uuid := 'c2c2c2c2-c2c2-42c2-82c2-c2c2c2c2c2c2';
  v_sess1 uuid := 'd1d1d1d1-d1d1-41d1-81d1-d1d1d1d1d1d1';
  v_sess2 uuid := 'd2d2d2d2-d2d2-42d2-82d2-d2d2d2d2d2d2';
  v_exp1 uuid := 'e1e1e1e1-e1e1-41e1-81e1-e1e1e1e1e1e1';
  v_exp2 uuid := 'e2e2e2e2-e2e2-42e2-82e2-e2e2e2e2e2e2';
  v_shift1 uuid := 'f1f1f1f1-f1f1-41f1-81f1-f1f1f1f1f1f1';
  v_shift2 uuid := 'f2f2f2f2-f2f2-42f2-82f2-f2f2f2f2f2f2';
  v_set_b2 uuid := '01010101-0101-4101-8101-010101010101';
  v_set_global uuid := '02020202-0202-4202-8202-020202020202';
  v_new_order uuid := '03030303-0303-4303-8303-030303030303';
  v_seen int;
  v_qty int;
  v_result jsonb;
BEGIN
  INSERT INTO auth.users (id, email) VALUES
    (v_admin, 'admin@local'),
    (v_c1, 'c1@local'),
    (v_c2, 'c2@local'),
    (v_inactive, 'inactive@local')
  ON CONFLICT DO NOTHING;
  INSERT INTO branches (id, code, name) VALUES
    (v_b1, 'B1', 'Branch 1'),
    (v_b2, 'B2', 'Branch 2')
  ON CONFLICT (id) DO NOTHING;
  INSERT INTO user_profiles (user_id, display_name, is_system_admin, is_active) VALUES
    (v_admin, 'Admin', true, true),
    (v_c1, 'Cashier 1', false, true),
    (v_c2, 'Cashier 2', false, true),
    (v_inactive, 'Inactive', false, true)
  ON CONFLICT (user_id) DO UPDATE SET
    is_system_admin = EXCLUDED.is_system_admin,
    is_active = EXCLUDED.is_active;
  INSERT INTO user_branch_roles (user_id, branch_id, role, is_active) VALUES
    (v_admin, v_b1, 'admin', true),
    (v_c1, v_b1, 'cashier', true),
    (v_c2, v_b2, 'cashier', true),
    (v_inactive, v_b1, 'cashier', false)
  ON CONFLICT (user_id, branch_id) DO UPDATE SET is_active = EXCLUDED.is_active;
  INSERT INTO devices (id, branch_id, name, device_key, is_active) VALUES
    (v_d1, v_b1, 'Cashier B1', 'rls-dev-b1', true),
    (v_d2, v_b2, 'Cashier B2', 'rls-dev-b2', true)
  ON CONFLICT (id) DO NOTHING;
  INSERT INTO stations (id, branch_id, code, display_name) VALUES
    (v_s1, v_b1, 'RLS1', 'RLS PS1'),
    (v_s2, v_b2, 'RLS1', 'RLS PS1')
  ON CONFLICT (id) DO NOTHING;
  INSERT INTO categories (id, name) VALUES (v_cat, 'Drinks')
  ON CONFLICT (id) DO NOTHING;
  INSERT INTO products (id, category_id, sku, name, default_sell_price_minor, default_cost_price_minor)
  VALUES (v_prod, v_cat, 'RLS-COKE', 'Coke', 1500, 700)
  ON CONFLICT (id) DO NOTHING;
  INSERT INTO branch_products (branch_id, product_id, updated_at) VALUES
    (v_b1, v_prod, now()),
    (v_b2, v_prod, now())
  ON CONFLICT (branch_id, product_id) DO NOTHING;
  INSERT INTO inventory_balances (branch_id, product_id, quantity_on_hand, version, updated_at) VALUES
    (v_b1, v_prod, 10, 0, now()),
    (v_b2, v_prod, 99, 0, now())
  ON CONFLICT (branch_id, product_id) DO UPDATE SET quantity_on_hand = EXCLUDED.quantity_on_hand;
  INSERT INTO orders (
    id, branch_id, order_type, status, currency_code, opened_by, opened_at
  ) VALUES
    (v_o1, v_b1, 'pos', 'open', 'EGP', v_c1, now()),
    (v_o2, v_b2, 'pos', 'open', 'EGP', v_c2, now()),
    (v_og1, v_b1, 'gaming', 'open', 'EGP', v_c1, now()),
    (v_og2, v_b2, 'gaming', 'open', 'EGP', v_c2, now())
  ON CONFLICT (id) DO NOTHING;
  INSERT INTO order_items (
    id, branch_id, order_id, product_id, product_name_snapshot, quantity,
    unit_price_minor, unit_cost_minor, line_total_minor, added_by, added_at
  ) VALUES
    (v_item1, v_b1, v_o1, v_prod, 'Coke', 1, 1500, 700, 1500, v_c1, now()),
    (v_item2, v_b2, v_o2, v_prod, 'Coke', 1, 1500, 700, 1500, v_c2, now())
  ON CONFLICT (id) DO NOTHING;
  INSERT INTO payments (
    id, branch_id, order_id, payment_method_id, payment_type,
    amount_due_minor, amount_tendered_minor, amount_applied_minor, change_minor,
    status, cashier_id, paid_at, origin_event_id
  ) VALUES
    (v_pay1, v_b1, v_o1, '11111111-1111-1111-1111-111111111111', 'sale',
     1500, 1500, 1500, 0, 'captured', v_c1, now(), gen_random_uuid()),
    (v_pay2, v_b2, v_o2, '11111111-1111-1111-1111-111111111111', 'sale',
     1500, 1500, 1500, 0, 'captured', v_c2, now(), gen_random_uuid())
  ON CONFLICT (id) DO NOTHING;
  INSERT INTO gaming_sessions (
    id, branch_id, station_id, order_id, status, started_at, pricing_snapshot, started_by
  ) VALUES
    (v_sess1, v_b1, v_s1, v_og1, 'active', now(), '{}'::jsonb, v_c1),
    (v_sess2, v_b2, v_s2, v_og2, 'active', now(), '{}'::jsonb, v_c2)
  ON CONFLICT (id) DO NOTHING;
  INSERT INTO expenses (id, branch_id, category, amount_minor, expense_at, created_by) VALUES
    (v_exp1, v_b1, 'supplies', 100, now(), v_admin),
    (v_exp2, v_b2, 'supplies', 200, now(), v_admin)
  ON CONFLICT (id) DO NOTHING;
  INSERT INTO cashier_shifts (
    id, branch_id, user_id, device_id, status, opening_cash_minor, started_at
  ) VALUES
    (v_shift1, v_b1, v_c1, v_d1, 'open', 0, now()),
    (v_shift2, v_b2, v_c2, v_d2, 'open', 0, now())
  ON CONFLICT (id) DO NOTHING;
  INSERT INTO app_settings (id, scope, branch_id, key, value) VALUES
    (v_set_global, 'global', NULL, 'locale', '{"default":"ar"}'::jsonb),
    (v_set_b2, 'branch', v_b2, 'receipt_prefix', '{"v":"B2"}'::jsonb)
  ON CONFLICT (id) DO NOTHING;
  INSERT INTO inventory_movements (
    id, branch_id, product_id, movement_type, quantity_delta, quantity_after,
    origin_event_id, created_by, created_at
  ) VALUES
    (gen_random_uuid(), v_b1, v_prod, 'opening', 10, 10, gen_random_uuid(), v_admin, now()),
    (gen_random_uuid(), v_b2, v_prod, 'opening', 99, 99, gen_random_uuid(), v_admin, now());
  INSERT INTO sync_receipts (event_id, branch_id, device_id, local_sequence, event_type, payload_hash)
  VALUES
    (gen_random_uuid(), v_b1, v_d1, 90, 'order.opened', 'hash-b1'),
    (gen_random_uuid(), v_b2, v_d2, 90, 'order.opened', 'hash-b2')
  ON CONFLICT (event_id) DO NOTHING;

  -- Cashier B1: own branch visible, other branch hidden, catalog readable.
  PERFORM set_config('request.jwt.claim.sub', v_c1::text, true);
  SET LOCAL ROLE authenticated;

  SELECT COUNT(*) INTO v_seen FROM orders WHERE branch_id = v_b2;
  IF v_seen <> 0 THEN
    RAISE EXCEPTION 'cashier B1 must not see B2 orders';
  END IF;
  SELECT COUNT(*) INTO v_seen FROM orders WHERE branch_id = v_b1;
  IF v_seen < 1 THEN
    RAISE EXCEPTION 'cashier B1 must see B1 orders';
  END IF;
  SELECT COUNT(*) INTO v_seen FROM order_items WHERE branch_id = v_b2;
  IF v_seen <> 0 THEN
    RAISE EXCEPTION 'cashier B1 must not see B2 order_items';
  END IF;
  SELECT COUNT(*) INTO v_seen FROM payments WHERE branch_id = v_b2;
  IF v_seen <> 0 THEN
    RAISE EXCEPTION 'cashier B1 must not see B2 payments';
  END IF;
  SELECT COUNT(*) INTO v_seen FROM payments WHERE branch_id = v_b1;
  IF v_seen < 1 THEN
    RAISE EXCEPTION 'cashier B1 must see B1 payments';
  END IF;
  SELECT COUNT(*) INTO v_seen FROM gaming_sessions WHERE branch_id = v_b2;
  IF v_seen <> 0 THEN
    RAISE EXCEPTION 'cashier B1 must not see B2 sessions';
  END IF;
  SELECT quantity_on_hand INTO v_qty FROM inventory_balances WHERE branch_id = v_b2 AND product_id = v_prod;
  IF v_qty IS NOT NULL THEN
    RAISE EXCEPTION 'cashier B1 must not see B2 inventory';
  END IF;
  SELECT quantity_on_hand INTO v_qty FROM inventory_balances WHERE branch_id = v_b1 AND product_id = v_prod;
  IF v_qty <> 10 THEN
    RAISE EXCEPTION 'cashier B1 must see B1 inventory';
  END IF;
  SELECT COUNT(*) INTO v_seen FROM inventory_movements WHERE branch_id = v_b2;
  IF v_seen <> 0 THEN
    RAISE EXCEPTION 'cashier B1 must not see B2 inventory_movements';
  END IF;
  SELECT COUNT(*) INTO v_seen FROM expenses WHERE branch_id = v_b2;
  IF v_seen <> 0 THEN
    RAISE EXCEPTION 'cashier B1 must not see B2 expenses';
  END IF;
  SELECT COUNT(*) INTO v_seen FROM cashier_shifts WHERE branch_id = v_b2;
  IF v_seen <> 0 THEN
    RAISE EXCEPTION 'cashier B1 must not see B2 shifts';
  END IF;
  SELECT COUNT(*) INTO v_seen FROM sync_receipts WHERE branch_id = v_b2;
  IF v_seen <> 0 THEN
    RAISE EXCEPTION 'cashier B1 must not see B2 sync_receipts';
  END IF;
  SELECT COUNT(*) INTO v_seen FROM devices WHERE branch_id = v_b2;
  IF v_seen <> 0 THEN
    RAISE EXCEPTION 'cashier B1 must not see B2 devices';
  END IF;
  SELECT COUNT(*) INTO v_seen FROM stations WHERE branch_id = v_b2;
  IF v_seen <> 0 THEN
    RAISE EXCEPTION 'cashier B1 must not see B2 stations';
  END IF;
  SELECT COUNT(*) INTO v_seen FROM branch_products WHERE branch_id = v_b2;
  IF v_seen <> 0 THEN
    RAISE EXCEPTION 'cashier B1 must not see B2 branch_products';
  END IF;
  SELECT COUNT(*) INTO v_seen FROM app_settings WHERE id = v_set_b2;
  IF v_seen <> 0 THEN
    RAISE EXCEPTION 'cashier B1 must not see B2 app_settings';
  END IF;
  SELECT COUNT(*) INTO v_seen FROM app_settings WHERE id = v_set_global;
  IF v_seen <> 1 THEN
    RAISE EXCEPTION 'cashier B1 must see global app_settings';
  END IF;
  SELECT COUNT(*) INTO v_seen FROM products WHERE id = v_prod;
  IF v_seen <> 1 THEN
    RAISE EXCEPTION 'cashier must read the product catalog';
  END IF;
  SELECT COUNT(*) INTO v_seen FROM user_profiles WHERE user_id = v_admin;
  IF v_seen <> 0 THEN
    RAISE EXCEPTION 'cashier must not read another profile';
  END IF;
  SELECT COUNT(*) INTO v_seen FROM user_profiles WHERE user_id = v_c1;
  IF v_seen <> 1 THEN
    RAISE EXCEPTION 'cashier must read their own profile';
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
    format('UPDATE orders SET status = %L WHERE id = %L', 'paid', v_o1),
    'cashier direct order update'
  );
  PERFORM public.test_rls_expect_deny(
    format('DELETE FROM orders WHERE id = %L', v_o1),
    'cashier direct order delete'
  );
  PERFORM public.test_rls_expect_deny(
    format('INSERT INTO payments (id, branch_id, order_id, payment_method_id, payment_type, amount_due_minor, amount_tendered_minor, amount_applied_minor, change_minor, status, cashier_id, paid_at, origin_event_id) VALUES (%L, %L, %L, %L, %L, 1, 1, 1, 0, %L, %L, now(), %L)',
      gen_random_uuid(), v_b1, v_o1, '11111111-1111-1111-1111-111111111111', 'sale', 'captured', v_c1, gen_random_uuid()),
    'cashier direct payment insert'
  );
  PERFORM public.test_rls_expect_deny(
    format('UPDATE inventory_balances SET quantity_on_hand = 0 WHERE branch_id = %L AND product_id = %L', v_b1, v_prod),
    'cashier direct inventory update'
  );
  PERFORM public.test_rls_expect_deny(
    format('INSERT INTO products (id, category_id, sku, name, default_sell_price_minor, default_cost_price_minor) VALUES (%L, %L, %L, %L, 100, 50)',
      gen_random_uuid(), v_cat, 'RLS-DENY', 'Denied'),
    'cashier catalog insert'
  );
  PERFORM public.test_rls_expect_deny(
    format('INSERT INTO branches (code, name) VALUES (%L, %L)', 'BX', 'Denied'),
    'cashier branch insert'
  );
  PERFORM public.test_rls_expect_deny(
    format('UPDATE app_settings SET value = %L WHERE id = %L', '{"x":1}', v_set_global),
    'cashier settings update'
  );

  BEGIN
    PERFORM public.pull_branch_since(v_b2, now() - interval '1 day');
    RAISE EXCEPTION 'cashier B1 pull of B2 must fail';
  EXCEPTION
    WHEN insufficient_privilege THEN
      NULL;
    WHEN others THEN
      IF SQLERRM NOT LIKE '%branch_forbidden%' THEN
        RAISE;
      END IF;
  END;

  BEGIN
    PERFORM public.apply_domain_event(
      gen_random_uuid(), v_b2, v_d2, 1, 'order.opened',
      jsonb_build_object('order_id', gen_random_uuid(), 'order_type', 'pos', 'opened_by', v_c1, 'opened_at', now()),
      'hash-forbidden'
    );
    RAISE EXCEPTION 'cashier B1 apply on B2 must fail';
  EXCEPTION
    WHEN insufficient_privilege THEN
      NULL;
    WHEN others THEN
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
    'hash-rls-open'
  ) INTO v_result;
  IF v_result->>'status' <> 'applied' THEN
    RAISE EXCEPTION 'cashier B1 SECURITY DEFINER apply on own branch must work';
  END IF;
  RESET ROLE;

  -- Cashier B2 cannot see B1.
  PERFORM set_config('request.jwt.claim.sub', v_c2::text, true);
  SET LOCAL ROLE authenticated;
  SELECT COUNT(*) INTO v_seen FROM orders WHERE branch_id = v_b1;
  IF v_seen <> 0 THEN
    RAISE EXCEPTION 'cashier B2 must not see B1 orders';
  END IF;
  SELECT COUNT(*) INTO v_seen FROM orders WHERE branch_id = v_b2;
  IF v_seen < 1 THEN
    RAISE EXCEPTION 'cashier B2 must see B2 orders';
  END IF;
  RESET ROLE;

  -- Inactive B1 cashier is denied the branch.
  PERFORM set_config('request.jwt.claim.sub', v_inactive::text, true);
  SET LOCAL ROLE authenticated;
  SELECT COUNT(*) INTO v_seen FROM orders WHERE branch_id = v_b1;
  IF v_seen <> 0 THEN
    RAISE EXCEPTION 'inactive cashier must not see B1 orders';
  END IF;
  RESET ROLE;

  -- System admin sees both branches and may write catalog, not money tables.
  PERFORM set_config('request.jwt.claim.sub', v_admin::text, true);
  SET LOCAL ROLE authenticated;
  SELECT COUNT(*) INTO v_seen FROM orders WHERE branch_id IN (v_b1, v_b2);
  IF v_seen < 2 THEN
    RAISE EXCEPTION 'admin must see both branches';
  END IF;
  SELECT COUNT(*) INTO v_seen FROM inventory_balances;
  IF v_seen < 2 THEN
    RAISE EXCEPTION 'admin must see both inventory rows';
  END IF;
  SELECT COUNT(*) INTO v_seen FROM payments WHERE branch_id IN (v_b1, v_b2);
  IF v_seen < 2 THEN
    RAISE EXCEPTION 'admin must see both payments';
  END IF;
  INSERT INTO categories (name, sort_order) VALUES ('Admin Catalog', 99);
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
  PERFORM set_config('request.jwt.claim.sub', '', true);
  SET LOCAL ROLE anon;
  PERFORM public.test_rls_expect_deny('SELECT COUNT(*) FROM orders', 'anon orders');
  PERFORM public.test_rls_expect_deny('SELECT COUNT(*) FROM payments', 'anon payments');
  PERFORM public.test_rls_expect_deny('SELECT COUNT(*) FROM inventory_balances', 'anon inventory');
  PERFORM public.test_rls_expect_deny('SELECT COUNT(*) FROM user_profiles', 'anon profiles');
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
END;
$$;

SELECT public.test_rls_branch_isolation();
