-- Hosted apply_domain_event acceptance. Disposable HTE identities only.
-- Requires auth_insert_helper.sql first. Do not add to scripts/run-pg-tests.sh.

CREATE OR REPLACE FUNCTION public.hosted_cleanup_event_acceptance()
RETURNS void
LANGUAGE plpgsql
AS $$
DECLARE
  v_branch uuid := 'aa020001-0000-4000-8000-0000000000e1';
  v_user uuid := 'aa0200c1-0000-4000-8000-0000000000c1';
  v_prod uuid := 'aa0200f3-0000-4000-8000-0000000000f3';
  v_cat uuid := 'aa0200f2-0000-4000-8000-0000000000f2';
BEGIN
  RESET ROLE;
  PERFORM public.hosted_set_jwt(NULL);

  DELETE FROM audit_logs WHERE branch_id = v_branch;
  DELETE FROM sync_receipts WHERE branch_id = v_branch;
  DELETE FROM device_sequence_cloud WHERE device_id = 'aa0200d1-0000-4000-8000-0000000000d1';
  DELETE FROM inventory_movements WHERE branch_id = v_branch;
  DELETE FROM payments WHERE branch_id = v_branch;
  DELETE FROM order_items WHERE branch_id = v_branch;
  DELETE FROM gaming_sessions WHERE branch_id = v_branch;
  DELETE FROM orders WHERE branch_id = v_branch;
  DELETE FROM inventory_balances WHERE branch_id = v_branch;
  DELETE FROM branch_products WHERE branch_id = v_branch;
  DELETE FROM pricing_rules WHERE branch_id = v_branch;
  DELETE FROM stations WHERE branch_id = v_branch;
  DELETE FROM devices WHERE branch_id = v_branch;
  DELETE FROM user_branch_roles WHERE branch_id = v_branch;
  DELETE FROM products WHERE id = v_prod;
  DELETE FROM categories WHERE id = v_cat;
  DELETE FROM user_profiles WHERE user_id = v_user;
  DELETE FROM branches WHERE id = v_branch AND code = 'HTE';
  PERFORM public.hosted_delete_auth_user(v_user);
END;
$$;

CREATE OR REPLACE FUNCTION public.test_hosted_event_acceptance()
RETURNS void
LANGUAGE plpgsql
AS $$
DECLARE
  v_branch uuid := 'aa020001-0000-4000-8000-0000000000e1';
  v_device uuid := 'aa0200d1-0000-4000-8000-0000000000d1';
  v_user uuid := 'aa0200c1-0000-4000-8000-0000000000c1';
  v_station uuid := 'aa0200e1-0000-4000-8000-0000000000e1';
  v_rule uuid := 'aa0200f1-0000-4000-8000-0000000000f1';
  v_cat uuid := 'aa0200f2-0000-4000-8000-0000000000f2';
  v_prod uuid := 'aa0200f3-0000-4000-8000-0000000000f3';
  v_order uuid := 'aa020101-0000-4000-8000-000000000101';
  v_session uuid := 'aa020141-0000-4000-8000-000000000141';
  v_item uuid := 'aa020121-0000-4000-8000-000000000121';
  v_pay uuid := 'aa020131-0000-4000-8000-000000000131';
  v_rev uuid := 'aa020132-0000-4000-8000-000000000132';
  v_e1 uuid := 'aa020201-0000-4000-8000-000000000201';
  v_e2 uuid := 'aa020202-0000-4000-8000-000000000202';
  v_e3 uuid := 'aa020203-0000-4000-8000-000000000203';
  v_e4 uuid := 'aa020204-0000-4000-8000-000000000204';
  v_e5 uuid := 'aa020205-0000-4000-8000-000000000205';
  v_e6 uuid := 'aa020206-0000-4000-8000-000000000206';
  v_e7 uuid := 'aa020207-0000-4000-8000-000000000207';
  v_result jsonb;
  v_status text;
  v_payments int;
  v_receipt text;
  v_snap jsonb;
  v_cap jsonb;
  v_paid jsonb;
  v_linear jsonb;
  v_stepped jsonb;
  v_stop jsonb;
  v_item_payload jsonb;
  v_rev_payload jsonb;
BEGIN
  PERFORM public.hosted_cleanup_event_acceptance();

  PERFORM public.hosted_insert_auth_user(v_user, 'ht-event@hosted-test.invalid');
  INSERT INTO branches (id, code, name) VALUES (v_branch, 'HTE', 'Hosted event test');
  INSERT INTO user_profiles (user_id, display_name, is_system_admin, is_active)
  VALUES (v_user, 'HT Event cashier', false, true);
  INSERT INTO user_branch_roles (user_id, branch_id, role, is_active)
  VALUES (v_user, v_branch, 'cashier', true);
  INSERT INTO devices (id, branch_id, name, device_key, is_active)
  VALUES (v_device, v_branch, 'HT Event device', 'ht-dev-hte', true);
  INSERT INTO stations (id, branch_id, code, display_name)
  VALUES (v_station, v_branch, 'HTE1', 'HT Event PS');
  INSERT INTO pricing_rules (id, branch_id, name, rule_type, rate_minor_per_hour, effective_from)
  VALUES (v_rule, v_branch, 'HT Linear', 'linear', 3600, now());
  INSERT INTO categories (id, name) VALUES (v_cat, 'HT Event drinks');
  INSERT INTO products (id, category_id, sku, name, default_sell_price_minor, default_cost_price_minor)
  VALUES (v_prod, v_cat, 'HT-EVT-COKE', 'HT Event Coke', 1500, 700);
  INSERT INTO branch_products (branch_id, product_id, updated_at) VALUES (v_branch, v_prod, now());
  INSERT INTO inventory_balances (branch_id, product_id, quantity_on_hand, version, updated_at)
  VALUES (v_branch, v_prod, 10, 0, now());

  PERFORM public.hosted_set_jwt(v_user);

  v_linear := jsonb_build_object(
    'session_id', v_session,
    'station_id', v_station,
    'order_id', v_order,
    'started_at', now(),
    'pricing_rule_id', v_rule,
    'started_by', v_user,
    'pricing_snapshot', jsonb_build_object(
      'rule_type', 'linear',
      'rate_minor_per_hour', 3600
    )
  );
  v_stepped := v_linear || jsonb_build_object(
    'pricing_snapshot', jsonb_build_object(
      'rule_type', 'stepped',
      'rate_minor_per_hour', 3600
    )
  );
  v_stop := jsonb_build_object(
    'session_id', v_session,
    'ended_at', now(),
    'duration_seconds', 3600,
    'calculated_charge_minor', 3600,
    'final_charge_minor', 3600,
    'stopped_by', v_user
  );
  v_item_payload := jsonb_build_object(
    'order_item_id', v_item,
    'order_id', v_order,
    'product_id', v_prod,
    'product_name_snapshot', 'HT Event Coke',
    'quantity', 1,
    'unit_price_minor', 1500,
    'unit_cost_minor', 700,
    'line_total_minor', 1500,
    'added_by', v_user,
    'added_at', now()
  );
  v_cap := jsonb_build_object(
    'payment_id', v_pay,
    'order_id', v_order,
    'branch_id', v_branch,
    'payment_method_id', '11111111-1111-1111-1111-111111111111',
    'amount_due_minor', 5100,
    'amount_tendered_minor', 10000,
    'amount_applied_minor', 5100,
    'change_minor', 4900,
    'cashier_id', v_user,
    'paid_at', now()
  );
  v_paid := v_cap || jsonb_build_object(
    'receipt_number', 'HTE-TEST-0001',
    'receipt_snapshot', jsonb_build_object(
      'tax_minor', 0,
      'tax_rate_bps', 0,
      'subtotal_minor', 5100,
      'total_minor', 5100
    ),
    'closed_by', v_user,
    'closed_at', now(),
    'total_minor', 5100,
    'subtotal_minor', 5100,
    'tax_minor', 0,
    'tax_rate_bps', 0,
    'discount_minor', 0
  );
  v_rev_payload := jsonb_build_object(
    'payment_id', v_rev,
    'parent_payment_id', v_pay,
    'order_id', v_order,
    'branch_id', v_branch,
    'amount_applied_minor', 5100,
    'reversed_by', v_user,
    'reason', 'hosted acceptance reverse'
  );

  SELECT public.apply_domain_event(
    v_e1, v_branch, v_device, 1, 'order.opened',
    jsonb_build_object(
      'order_id', v_order,
      'order_type', 'gaming',
      'opened_by', v_user,
      'opened_at', now()
    ),
    'hash-hte-open'
  ) INTO v_result;
  IF v_result->>'status' <> 'applied' THEN
    RAISE EXCEPTION 'order.opened must apply';
  END IF;

  BEGIN
    PERFORM public.apply_domain_event(
      gen_random_uuid(), v_branch, v_device, 2, 'session.started', v_stepped, 'hash-hte-stepped'
    );
    RAISE EXCEPTION 'stepped session.started must fail';
  EXCEPTION
    WHEN others THEN
      IF SQLERRM LIKE '%stepped session.started must fail%' THEN
        RAISE;
      END IF;
      IF SQLERRM NOT LIKE '%mvp_linear_pricing_required%' THEN
        RAISE;
      END IF;
  END;

  SELECT public.apply_domain_event(
    v_e2, v_branch, v_device, 2, 'session.started', v_linear, 'hash-hte-linear'
  ) INTO v_result;
  IF v_result->>'status' <> 'applied' THEN
    RAISE EXCEPTION 'linear session.started must apply';
  END IF;

  SELECT public.apply_domain_event(
    v_e3, v_branch, v_device, 3, 'session.stopped', v_stop, 'hash-hte-stop'
  ) INTO v_result;
  IF v_result->>'status' <> 'applied' THEN
    RAISE EXCEPTION 'session.stopped must apply';
  END IF;

  SELECT public.apply_domain_event(
    v_e4, v_branch, v_device, 4, 'order.item_added', v_item_payload, 'hash-hte-item'
  ) INTO v_result;
  IF v_result->>'status' <> 'applied' THEN
    RAISE EXCEPTION 'order.item_added must apply';
  END IF;

  SELECT public.apply_domain_event(
    v_e5, v_branch, v_device, 5, 'payment.captured', v_cap, 'hash-hte-cap'
  ) INTO v_result;
  IF v_result->>'status' <> 'applied' THEN
    RAISE EXCEPTION 'payment.captured must apply';
  END IF;
  SELECT status INTO v_status FROM orders WHERE id = v_order;
  SELECT COUNT(*) INTO v_payments FROM payments WHERE order_id = v_order;
  IF v_status = 'paid' THEN
    RAISE EXCEPTION 'payment.captured must not mark paid';
  END IF;
  IF v_payments <> 0 THEN
    RAISE EXCEPTION 'payment.captured must not insert a captured sale';
  END IF;

  BEGIN
    PERFORM public.apply_domain_event(
      gen_random_uuid(), v_branch, v_device, 7, 'order.paid', v_paid, 'hash-hte-gap'
    );
    RAISE EXCEPTION 'sequence gap must fail';
  EXCEPTION
    WHEN others THEN
      IF SQLERRM LIKE '%sequence gap must fail%' THEN
        RAISE;
      END IF;
      IF SQLERRM NOT LIKE '%sequence_gap%' THEN
        RAISE;
      END IF;
  END;

  BEGIN
    PERFORM public.apply_domain_event(
      gen_random_uuid(),
      '99999999-9999-4999-8999-999999999999',
      v_device,
      6,
      'order.paid',
      v_paid,
      'hash-hte-branch'
    );
    RAISE EXCEPTION 'wrong branch must fail';
  EXCEPTION
    WHEN others THEN
      IF SQLERRM LIKE '%wrong branch must fail%' THEN
        RAISE;
      END IF;
  END;

  BEGIN
    PERFORM public.apply_domain_event(
      gen_random_uuid(), v_branch, v_device, 6, 'order.paid',
      v_paid || jsonb_build_object('amount_applied_minor', 100),
      'hash-hte-amt'
    );
    RAISE EXCEPTION 'amount mismatch must fail';
  EXCEPTION
    WHEN others THEN
      IF SQLERRM LIKE '%amount mismatch must fail%' THEN
        RAISE;
      END IF;
      IF SQLERRM NOT LIKE '%amount_mismatch%' THEN
        RAISE;
      END IF;
  END;

  SELECT public.apply_domain_event(
    v_e6, v_branch, v_device, 6, 'order.paid', v_paid, 'hash-hte-paid'
  ) INTO v_result;
  IF v_result->>'status' <> 'applied' THEN
    RAISE EXCEPTION 'order.paid must apply';
  END IF;
  SELECT status, receipt_number, receipt_snapshot
    INTO v_status, v_receipt, v_snap
  FROM orders WHERE id = v_order;
  SELECT COUNT(*) INTO v_payments
  FROM payments WHERE order_id = v_order AND payment_type = 'sale' AND status = 'captured';
  IF v_status <> 'paid' OR v_payments <> 1 THEN
    RAISE EXCEPTION 'order.paid must finalize exactly one sale';
  END IF;
  IF v_receipt IS DISTINCT FROM 'HTE-TEST-0001' OR v_snap IS NULL THEN
    RAISE EXCEPTION 'order.paid must store one receipt snapshot';
  END IF;

  BEGIN
    UPDATE orders SET tax_minor = tax_minor + 1 WHERE id = v_order;
    RAISE EXCEPTION 'paid receipt/tax must be immutable';
  EXCEPTION
    WHEN others THEN
      IF SQLERRM LIKE '%paid receipt/tax must be immutable%' THEN
        RAISE;
      END IF;
      IF SQLERRM NOT LIKE '%paid_tax_immutable%' THEN
        RAISE;
      END IF;
  END;

  SELECT public.apply_domain_event(
    v_e6, v_branch, v_device, 6, 'order.paid', v_paid, 'hash-hte-paid'
  ) INTO v_result;
  IF v_result->>'status' <> 'already_processed' THEN
    RAISE EXCEPTION 'duplicate order.paid must be already_processed';
  END IF;
  SELECT COUNT(*) INTO v_payments FROM payments WHERE order_id = v_order AND payment_type = 'sale';
  IF v_payments <> 1 THEN
    RAISE EXCEPTION 'timeout replay created a second payment';
  END IF;

  BEGIN
    PERFORM public.apply_domain_event(
      v_e6, v_branch, v_device, 6, 'order.paid', v_paid, 'hash-hte-paid-mismatch'
    );
    RAISE EXCEPTION 'payload mismatch must fail';
  EXCEPTION
    WHEN others THEN
      IF SQLERRM LIKE '%payload mismatch must fail%' THEN
        RAISE;
      END IF;
      IF SQLERRM NOT LIKE '%event_id_payload_mismatch%' THEN
        RAISE;
      END IF;
  END;

  SELECT public.apply_domain_event(
    v_e5, v_branch, v_device, 5, 'payment.captured', v_cap, 'hash-hte-cap'
  ) INTO v_result;
  IF v_result->>'status' <> 'already_processed' THEN
    RAISE EXCEPTION 'duplicate payment.captured must be already_processed';
  END IF;

  SELECT public.apply_domain_event(
    v_e7, v_branch, v_device, 7, 'payment.reversed', v_rev_payload, 'hash-hte-rev'
  ) INTO v_result;
  IF v_result->>'status' <> 'applied' THEN
    RAISE EXCEPTION 'payment.reversed must apply';
  END IF;
  IF (SELECT status FROM payments WHERE id = v_pay) <> 'reversed' THEN
    RAISE EXCEPTION 'parent sale must be marked reversed';
  END IF;
  IF (SELECT COUNT(*) FROM payments WHERE id = v_rev AND payment_type = 'reversal') <> 1 THEN
    RAISE EXCEPTION 'canonical reversal row missing';
  END IF;
  IF (SELECT status FROM orders WHERE id = v_order) <> 'checkout_pending' THEN
    RAISE EXCEPTION 'reverse must reopen checkout_pending';
  END IF;

  PERFORM public.hosted_cleanup_event_acceptance();
EXCEPTION
  WHEN others THEN
    BEGIN
      PERFORM public.hosted_cleanup_event_acceptance();
    EXCEPTION
      WHEN others THEN
        NULL;
    END;
    RAISE;
END;
$$;

SELECT public.test_hosted_event_acceptance();

DROP FUNCTION IF EXISTS public.test_hosted_event_acceptance();
DROP FUNCTION IF EXISTS public.hosted_cleanup_event_acceptance();
DROP FUNCTION IF EXISTS public.test_hosted_rls_matrix();
DROP FUNCTION IF EXISTS public.hosted_cleanup_rls_matrix();
DROP FUNCTION IF EXISTS public.test_rls_expect_deny(text, text);
DROP FUNCTION IF EXISTS public.hosted_delete_auth_user(uuid);
DROP FUNCTION IF EXISTS public.hosted_set_jwt(uuid);
DROP FUNCTION IF EXISTS public.hosted_insert_auth_user(uuid, text);
DROP FUNCTION IF EXISTS public.hosted_auth_user_placeholder(text, text);
