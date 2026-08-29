-- Live PostgreSQL contract for payment.captured vs order.paid atomicity.

CREATE OR REPLACE FUNCTION public.test_payment_atomicity()
RETURNS void
LANGUAGE plpgsql
AS $$
DECLARE
  v_branch uuid := '11111111-1111-4111-8111-111111111111';
  v_device uuid := '22222222-2222-4222-8222-222222222222';
  v_user uuid := '33333333-3333-4333-8333-333333333333';
  v_order uuid := '44444444-4444-4444-8444-444444444444';
  v_pay uuid := '55555555-5555-4555-8555-555555555555';
  v_e1 uuid := '66666666-6666-4666-8666-666666666666';
  v_e2 uuid := '77777777-7777-4777-8777-777777777777';
  v_e3 uuid := '88888888-8888-4888-8888-888888888888';
  v_cap jsonb;
  v_paid jsonb;
  v_rev jsonb;
  v_status text;
  v_result jsonb;
  v_payments int;
BEGIN
  INSERT INTO auth.users (id, email) VALUES (v_user, 'cashier@local') ON CONFLICT DO NOTHING;
  INSERT INTO branches (id, code, name) VALUES (v_branch, 'B1', 'Branch 1') ON CONFLICT (id) DO NOTHING;
  INSERT INTO user_profiles (user_id, display_name, is_system_admin, is_active)
  VALUES (v_user, 'Cashier', true, true)
  ON CONFLICT (user_id) DO UPDATE SET is_system_admin = true;
  INSERT INTO user_branch_roles (user_id, branch_id, role, is_active)
  VALUES (v_user, v_branch, 'admin', true)
  ON CONFLICT (user_id, branch_id) DO NOTHING;
  INSERT INTO devices (id, branch_id, name, device_key, is_active)
  VALUES (v_device, v_branch, 'Cashier 1', 'dev-key-b1', true)
  ON CONFLICT (id) DO NOTHING;
  INSERT INTO orders (
    id, branch_id, order_type, status, currency_code, opened_by, opened_at,
    product_subtotal_minor, gaming_subtotal_minor, subtotal_minor, total_minor,
    tax_minor, tax_rate_bps, discount_minor
  ) VALUES (
    v_order, v_branch, 'pos', 'checkout_pending', 'EGP', v_user, now(),
    5500, 0, 5500, 5500, 0, 0, 0
  ) ON CONFLICT (id) DO NOTHING;

  PERFORM set_config('request.jwt.claim.sub', v_user::text, true);

  v_cap := jsonb_build_object(
    'payment_id', v_pay,
    'order_id', v_order,
    'branch_id', v_branch,
    'payment_method_id', '11111111-1111-1111-1111-111111111111',
    'amount_due_minor', 5500,
    'amount_tendered_minor', 20000,
    'amount_applied_minor', 5500,
    'change_minor', 14500,
    'cashier_id', v_user,
    'paid_at', now()
  );
  v_paid := v_cap || jsonb_build_object(
    'receipt_number', 'B-TEST-0001',
    'receipt_snapshot', jsonb_build_object(
      'tax_minor', 0,
      'tax_rate_bps', 0,
      'subtotal_minor', 5500,
      'total_minor', 5500
    ),
    'closed_by', v_user,
    'closed_at', now(),
    'total_minor', 5500,
    'subtotal_minor', 5500,
    'tax_minor', 0,
    'tax_rate_bps', 0,
    'discount_minor', 0
  );
  v_rev := jsonb_build_object(
    'payment_id', gen_random_uuid(),
    'parent_payment_id', v_pay,
    'order_id', v_order,
    'branch_id', v_branch,
    'amount_applied_minor', 5500,
    'reversed_by', v_user,
    'reason', 'too soon'
  );

  PERFORM public.apply_domain_event(v_e1, v_branch, v_device, 1, 'payment.captured', v_cap, 'hash-cap');
  SELECT status INTO v_status FROM orders WHERE id = v_order;
  SELECT COUNT(*) INTO v_payments FROM payments WHERE order_id = v_order;
  IF v_status = 'paid' THEN
    RAISE EXCEPTION 'payment.captured must not mark paid';
  END IF;
  IF v_payments <> 0 THEN
    RAISE EXCEPTION 'payment.captured must not insert a captured sale';
  END IF;

  BEGIN
    PERFORM public.apply_domain_event(v_e3, v_branch, v_device, 2, 'payment.reversed', v_rev, 'hash-rev-early');
    RAISE EXCEPTION 'reverse before order.paid must fail';
  EXCEPTION
    WHEN others THEN
      IF SQLERRM NOT LIKE '%order_not_paid%' THEN
        RAISE;
      END IF;
  END;

  BEGIN
    PERFORM public.apply_domain_event(gen_random_uuid(), v_branch, v_device, 3, 'order.paid', v_paid, 'hash-gap');
    RAISE EXCEPTION 'sequence gap must fail';
  EXCEPTION
    WHEN others THEN
      IF SQLERRM NOT LIKE '%sequence_gap%' THEN
        RAISE;
      END IF;
  END;

  BEGIN
    PERFORM public.apply_domain_event(gen_random_uuid(), '99999999-9999-4999-8999-999999999999', v_device, 2, 'order.paid', v_paid, 'hash-branch');
    RAISE EXCEPTION 'wrong branch must fail';
  EXCEPTION
    WHEN others THEN
      NULL;
  END;

  BEGIN
    PERFORM public.apply_domain_event(
      gen_random_uuid(), v_branch, v_device, 2, 'order.paid',
      v_paid || jsonb_build_object('amount_applied_minor', 100),
      'hash-amt'
    );
    RAISE EXCEPTION 'amount mismatch must fail';
  EXCEPTION
    WHEN others THEN
      IF SQLERRM NOT LIKE '%amount_mismatch%' THEN
        RAISE;
      END IF;
  END;

  SELECT public.apply_domain_event(v_e1, v_branch, v_device, 1, 'payment.captured', v_cap, 'hash-cap') INTO v_result;
  IF v_result->>'status' <> 'already_processed' THEN
    RAISE EXCEPTION 'duplicate payment.captured must be already_processed';
  END IF;

  PERFORM public.apply_domain_event(v_e2, v_branch, v_device, 2, 'order.paid', v_paid, 'hash-paid');
  SELECT status INTO v_status FROM orders WHERE id = v_order;
  SELECT COUNT(*) INTO v_payments FROM payments WHERE order_id = v_order AND payment_type = 'sale' AND status = 'captured';
  IF v_status <> 'paid' OR v_payments <> 1 THEN
    RAISE EXCEPTION 'order.paid must finalize exactly one sale';
  END IF;
  IF (SELECT receipt_snapshot->>'tax_minor' FROM orders WHERE id = v_order) <> '0' THEN
    RAISE EXCEPTION 'tax snapshot must be copied';
  END IF;

  SELECT public.apply_domain_event(v_e2, v_branch, v_device, 2, 'order.paid', v_paid, 'hash-paid') INTO v_result;
  IF v_result->>'status' <> 'already_processed' THEN
    RAISE EXCEPTION 'duplicate order.paid must be already_processed';
  END IF;
  SELECT COUNT(*) INTO v_payments FROM payments WHERE order_id = v_order AND payment_type = 'sale';
  IF v_payments <> 1 THEN
    RAISE EXCEPTION 'timeout replay created a second payment';
  END IF;
END;
$$;

SELECT public.test_payment_atomicity();
