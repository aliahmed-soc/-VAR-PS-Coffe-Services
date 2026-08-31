-- Live PostgreSQL contract for the event stream a whole-ticket void produces.
--
-- The till used to enqueue only order.voided, which flips the order to 'void'
-- and touches nothing else. Every line stayed 'active' and the units already
-- deducted were never credited back. Reproduced during physical UAT: a drink on
-- a voided walk-in ticket stayed missing from stock, and because the desktop had
-- the same gap local and cloud agreed on the wrong number.
--
-- A ticket void now retires each line through order.item_voided first, so the
-- cloud converges through the per-line handler, then closes with order.voided.
-- inventory_movements.origin_event_id is UNIQUE, so one event per line is also
-- the only shape the ledger accepts.

CREATE OR REPLACE FUNCTION public.test_order_void_returns_stock()
RETURNS void
LANGUAGE plpgsql
AS $$
DECLARE
  v_branch uuid := '1b1b1b1b-1b1b-4b1b-8b1b-1b1b1b1b1b1b';
  v_device uuid := '2b2b2b2b-2b2b-4b2b-8b2b-2b2b2b2b2b2b';
  v_user uuid := '3b3b3b3b-3b3b-4b3b-8b3b-3b3b3b3b3b3b';
  v_cat uuid := '4b4b4b4b-4b4b-4b4b-8b4b-4b4b4b4b4b4b';
  v_coke uuid := '5b5b5b5b-5b5b-4b5b-8b5b-5b5b5b5b5b5b';
  v_chips uuid := '6b6b6b6b-6b6b-4b6b-8b6b-6b6b6b6b6b6b';
  v_order uuid := '7b7b7b7b-7b7b-4b7b-8b7b-7b7b7b7b7b7b';
  v_item1 uuid := '8b8b8b8b-8b8b-4b8b-8b8b-8b8b8b8b8b8b';
  v_item2 uuid := '9b9b9b9b-9b9b-4b9b-8b9b-9b9b9b9b9b9b';
  v_e1 uuid := 'bcbcbcbc-bcbc-4bcb-8bcb-bcbcbcbcbcbc';
  v_e2 uuid := 'cdcdcdcd-cdcd-4cdc-8cdc-cdcdcdcdcdcd';
  v_e3 uuid := 'dededede-dede-4ded-8ded-dededededede';
  v_coke_qty int;
  v_chips_qty int;
  v_active int;
  v_movements int;
  v_status text;
  v_subtotal bigint;
  v_result jsonb;
BEGIN
  INSERT INTO auth.users (id, email) VALUES (v_user, 'void-cashier@local') ON CONFLICT DO NOTHING;
  INSERT INTO branches (id, code, name) VALUES (v_branch, 'BV', 'Void Branch')
  ON CONFLICT (id) DO NOTHING;
  INSERT INTO user_profiles (user_id, display_name, is_system_admin, is_active)
  VALUES (v_user, 'Void Cashier', true, true)
  ON CONFLICT (user_id) DO UPDATE SET is_system_admin = true;
  INSERT INTO user_branch_roles (user_id, branch_id, role, is_active)
  VALUES (v_user, v_branch, 'admin', true)
  ON CONFLICT (user_id, branch_id) DO NOTHING;
  INSERT INTO devices (id, branch_id, name, device_key, is_active)
  VALUES (v_device, v_branch, 'Void Till', 'dev-key-void', true)
  ON CONFLICT (id) DO NOTHING;
  INSERT INTO categories (id, name) VALUES (v_cat, 'Void Drinks')
  ON CONFLICT (id) DO NOTHING;
  INSERT INTO products (
    id, category_id, sku, name, default_sell_price_minor, default_cost_price_minor, is_active
  )
  VALUES (v_coke, v_cat, 'VOID-COKE', 'Void Coke', 1000, 600, true),
         (v_chips, v_cat, 'VOID-CHIPS', 'Void Chips', 500, 300, true)
  ON CONFLICT (id) DO NOTHING;
  INSERT INTO inventory_balances (branch_id, product_id, quantity_on_hand, version)
  VALUES (v_branch, v_coke, 18, 1), (v_branch, v_chips, 17, 1)
  ON CONFLICT (branch_id, product_id) DO UPDATE SET quantity_on_hand = excluded.quantity_on_hand;

  -- A ticket that already consumed 2 coke and 3 chips.
  INSERT INTO orders (
    id, branch_id, order_type, status, currency_code, opened_by, opened_at,
    product_subtotal_minor, gaming_subtotal_minor, subtotal_minor, total_minor,
    tax_minor, tax_rate_bps, discount_minor
  ) VALUES (
    v_order, v_branch, 'pos', 'open', 'EGP', v_user, now(),
    3500, 0, 3500, 3500, 0, 0, 0
  ) ON CONFLICT (id) DO NOTHING;
  INSERT INTO order_items (
    id, order_id, branch_id, product_id, product_name_snapshot, quantity,
    unit_price_minor, unit_cost_minor, line_total_minor, status, added_by, added_at
  ) VALUES
    (v_item1, v_order, v_branch, v_coke, 'Void Coke', 2, 1000, 600, 2000, 'active', v_user, now()),
    (v_item2, v_order, v_branch, v_chips, 'Void Chips', 3, 500, 300, 1500, 'active', v_user, now())
  ON CONFLICT (id) DO NOTHING;

  PERFORM set_config('request.jwt.claim.sub', v_user::text, true);

  PERFORM public.apply_domain_event(
    v_e1, v_branch, v_device, 1, 'order.item_voided',
    jsonb_build_object(
      'order_item_id', v_item1,
      'order_id', v_order,
      'branch_id', v_branch,
      'quantity', 2,
      'voided_by', v_user,
      'void_reason', 'mistyped ticket'
    ),
    'hash-void-1'
  );
  PERFORM public.apply_domain_event(
    v_e2, v_branch, v_device, 2, 'order.item_voided',
    jsonb_build_object(
      'order_item_id', v_item2,
      'order_id', v_order,
      'branch_id', v_branch,
      'quantity', 3,
      'voided_by', v_user,
      'void_reason', 'mistyped ticket'
    ),
    'hash-void-2'
  );
  PERFORM public.apply_domain_event(
    v_e3, v_branch, v_device, 3, 'order.voided',
    jsonb_build_object(
      'order_id', v_order,
      'branch_id', v_branch,
      'voided_by', v_user,
      'reason', 'mistyped ticket'
    ),
    'hash-void-order'
  );

  SELECT quantity_on_hand INTO v_coke_qty FROM inventory_balances
  WHERE branch_id = v_branch AND product_id = v_coke;
  SELECT quantity_on_hand INTO v_chips_qty FROM inventory_balances
  WHERE branch_id = v_branch AND product_id = v_chips;
  IF v_coke_qty <> 20 OR v_chips_qty <> 20 THEN
    RAISE EXCEPTION 'a voided ticket must return its stock, got coke=% chips=%',
      v_coke_qty, v_chips_qty;
  END IF;

  SELECT COUNT(*) INTO v_active FROM order_items
  WHERE order_id = v_order AND status = 'active';
  IF v_active <> 0 THEN
    RAISE EXCEPTION 'a void ticket must hold no active line, % left', v_active;
  END IF;

  SELECT status, product_subtotal_minor INTO v_status, v_subtotal FROM orders WHERE id = v_order;
  IF v_status <> 'void' THEN
    RAISE EXCEPTION 'order must be void, got %', v_status;
  END IF;
  IF v_subtotal <> 0 THEN
    RAISE EXCEPTION 'a void ticket carries no product value, got %', v_subtotal;
  END IF;

  SELECT COUNT(*) INTO v_movements FROM inventory_movements
  WHERE order_id = v_order AND movement_type = 'sale_void';
  IF v_movements <> 2 THEN
    RAISE EXCEPTION 'expected one sale_void movement per line, got %', v_movements;
  END IF;
  IF EXISTS (
    SELECT 1 FROM inventory_movements
    WHERE order_id = v_order AND movement_type = 'sale_void'
      AND origin_event_id NOT IN (v_e1, v_e2)
  ) THEN
    RAISE EXCEPTION 'every credit must point at the line void that caused it';
  END IF;

  -- Replay must not credit the same line twice.
  SELECT public.apply_domain_event(
    v_e1, v_branch, v_device, 1, 'order.item_voided',
    jsonb_build_object(
      'order_item_id', v_item1,
      'order_id', v_order,
      'branch_id', v_branch,
      'quantity', 2,
      'voided_by', v_user,
      'void_reason', 'mistyped ticket'
    ),
    'hash-void-1'
  ) INTO v_result;
  IF v_result->>'status' <> 'already_processed' THEN
    RAISE EXCEPTION 'replayed line void must be already_processed, got %', v_result->>'status';
  END IF;
  SELECT quantity_on_hand INTO v_coke_qty FROM inventory_balances
  WHERE branch_id = v_branch AND product_id = v_coke;
  IF v_coke_qty <> 20 THEN
    RAISE EXCEPTION 'replayed line void credited stock twice, got %', v_coke_qty;
  END IF;
  SELECT COUNT(*) INTO v_movements FROM inventory_movements
  WHERE order_id = v_order AND movement_type = 'sale_void';
  IF v_movements <> 2 THEN
    RAISE EXCEPTION 'replay wrote a third movement, got %', v_movements;
  END IF;
END;
$$;

SELECT public.test_order_void_returns_stock();
