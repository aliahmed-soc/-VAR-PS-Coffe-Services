-- Voiding a whole ticket has to return its lines to stock. apply_domain_event
-- only flipped the order to 'void', so every line stayed 'active' and the units
-- already deducted were never credited back. A cashier voiding a mistyped ticket
-- lost that stock permanently, and because the desktop had the same gap local and
-- cloud agreed on the wrong number, so nothing flagged it.
--
-- Only the order.voided branch changes. The per-line order.item_voided path
-- already did this correctly and is left alone.

-- A reversal retires the order's receipt so the corrected repayment can store a
-- new one. Without this, apply_domain_event('order.paid') always raised
-- receipt_already_stored on the second payment: the desktop till closed the
-- order locally with a fresh receipt number while the event retried forever and
-- the cloud stayed stuck in checkout_pending on the retired receipt.
--
-- Only the payment.reversed branch changes. The order.paid receipt-immutability
-- guard is deliberately left in place.

-- Idempotent domain-event apply. Clients must not write financial tables directly.

CREATE OR REPLACE FUNCTION public.apply_domain_event(
  p_event_id uuid,
  p_branch_id uuid,
  p_device_id uuid,
  p_local_sequence bigint,
  p_event_type text,
  p_payload jsonb,
  p_payload_hash text
)
RETURNS jsonb
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public
AS $$
DECLARE
  v_last bigint;
  v_existing sync_receipts%ROWTYPE;
  v_device devices%ROWTYPE;
  v_order orders%ROWTYPE;
  v_applied bigint;
  v_tendered bigint;
  v_change bigint;
  v_total bigint;
  v_snap jsonb;
  v_snap_tax bigint;
  v_snap_rate integer;
  v_snap_sub bigint;
BEGIN
  IF auth.uid() IS NULL THEN
    RAISE EXCEPTION 'not_authenticated' USING ERRCODE = '28000';
  END IF;

  IF NOT public.has_branch_access(p_branch_id) THEN
    RAISE EXCEPTION 'branch_forbidden' USING ERRCODE = '42501';
  END IF;

  SELECT * INTO v_existing FROM sync_receipts WHERE event_id = p_event_id;
  IF FOUND THEN
    IF v_existing.payload_hash <> p_payload_hash THEN
      RAISE EXCEPTION 'event_id_payload_mismatch' USING ERRCODE = '22023';
    END IF;
    RETURN jsonb_build_object(
      'status', 'already_processed',
      'event_id', p_event_id,
      'local_sequence', v_existing.local_sequence
    );
  END IF;

  SELECT * INTO v_device FROM devices WHERE id = p_device_id AND is_active;
  IF NOT FOUND OR v_device.branch_id <> p_branch_id THEN
    RAISE EXCEPTION 'device_invalid' USING ERRCODE = '42501';
  END IF;

  INSERT INTO device_sequence_cloud(device_id, last_applied_sequence)
  VALUES (p_device_id, 0)
  ON CONFLICT (device_id) DO NOTHING;

  SELECT last_applied_sequence INTO v_last
  FROM device_sequence_cloud
  WHERE device_id = p_device_id
  FOR UPDATE;

  IF p_local_sequence <> v_last + 1 THEN
    RAISE EXCEPTION 'sequence_gap expected=% got=%', v_last + 1, p_local_sequence
      USING ERRCODE = 'P0001';
  END IF;

  CASE p_event_type
    WHEN 'order.opened' THEN
      INSERT INTO orders (
        id, branch_id, order_type, status, currency_code, opened_by, opened_at,
        tax_minor, tax_rate_bps, subtotal_minor, total_minor, origin_device_id
      ) VALUES (
        (p_payload->>'order_id')::uuid,
        p_branch_id,
        p_payload->>'order_type',
        'open',
        COALESCE(p_payload->>'currency_code', 'EGP'),
        (p_payload->>'opened_by')::uuid,
        (p_payload->>'opened_at')::timestamptz,
        0, 0, 0, 0, p_device_id
      );

    WHEN 'session.started' THEN
      IF COALESCE(p_payload->'pricing_snapshot'->>'rule_type', '') <> 'linear' THEN
        RAISE EXCEPTION 'mvp_linear_pricing_required';
      END IF;
      IF COALESCE((p_payload->'pricing_snapshot'->>'rate_minor_per_hour')::bigint, -1) < 0 THEN
        RAISE EXCEPTION 'mvp_linear_rate_required';
      END IF;
      INSERT INTO gaming_sessions (
        id, branch_id, station_id, order_id, status, started_at,
        pricing_rule_id, pricing_snapshot, started_by
      ) VALUES (
        (p_payload->>'session_id')::uuid,
        p_branch_id,
        (p_payload->>'station_id')::uuid,
        (p_payload->>'order_id')::uuid,
        'active',
        (p_payload->>'started_at')::timestamptz,
        (p_payload->>'pricing_rule_id')::uuid,
        p_payload->'pricing_snapshot',
        (p_payload->>'started_by')::uuid
      );

    WHEN 'session.stopped' THEN
      UPDATE gaming_sessions
      SET status = 'stopped',
          ended_at = (p_payload->>'ended_at')::timestamptz,
          duration_seconds = (p_payload->>'duration_seconds')::bigint,
          calculated_charge_minor = (p_payload->>'calculated_charge_minor')::bigint,
          final_charge_minor = (p_payload->>'final_charge_minor')::bigint,
          stopped_by = (p_payload->>'stopped_by')::uuid
      WHERE id = (p_payload->>'session_id')::uuid
        AND branch_id = p_branch_id
        AND status = 'active';
      UPDATE orders
      SET gaming_subtotal_minor = (p_payload->>'final_charge_minor')::bigint,
          subtotal_minor = product_subtotal_minor + (p_payload->>'final_charge_minor')::bigint,
          status = 'checkout_pending',
          total_minor = product_subtotal_minor + (p_payload->>'final_charge_minor')::bigint - discount_minor + tax_minor
      WHERE id = (SELECT order_id FROM gaming_sessions WHERE id = (p_payload->>'session_id')::uuid);

    WHEN 'session.resumed' THEN
      UPDATE gaming_sessions
      SET status = 'active',
          ended_at = NULL,
          duration_seconds = NULL,
          stopped_by = NULL
      WHERE id = (p_payload->>'session_id')::uuid
        AND branch_id = p_branch_id
        AND status = 'stopped';
      UPDATE orders
      SET status = 'open',
          gaming_subtotal_minor = 0,
          subtotal_minor = product_subtotal_minor,
          total_minor = product_subtotal_minor - discount_minor + tax_minor
      WHERE id = (SELECT order_id FROM gaming_sessions WHERE id = (p_payload->>'session_id')::uuid)
        AND status = 'checkout_pending';

    WHEN 'session.voided' THEN
      UPDATE gaming_sessions
      SET status = 'void'
      WHERE id = (p_payload->>'session_id')::uuid AND branch_id = p_branch_id;

    WHEN 'order.item_added' THEN
      INSERT INTO order_items (
        id, branch_id, order_id, product_id, product_name_snapshot, quantity,
        unit_price_minor, unit_cost_minor, line_total_minor, status, added_by, added_at
      ) VALUES (
        (p_payload->>'order_item_id')::uuid,
        p_branch_id,
        (p_payload->>'order_id')::uuid,
        (p_payload->>'product_id')::uuid,
        p_payload->>'product_name_snapshot',
        (p_payload->>'quantity')::integer,
        (p_payload->>'unit_price_minor')::bigint,
        (p_payload->>'unit_cost_minor')::bigint,
        (p_payload->>'line_total_minor')::bigint,
        'active',
        (p_payload->>'added_by')::uuid,
        COALESCE((p_payload->>'added_at')::timestamptz, now())
      );
      UPDATE orders
      SET product_subtotal_minor = product_subtotal_minor + (p_payload->>'line_total_minor')::bigint,
          subtotal_minor = product_subtotal_minor + (p_payload->>'line_total_minor')::bigint + gaming_subtotal_minor,
          total_minor = product_subtotal_minor + (p_payload->>'line_total_minor')::bigint + gaming_subtotal_minor - discount_minor + tax_minor
      WHERE id = (p_payload->>'order_id')::uuid AND branch_id = p_branch_id;
      INSERT INTO inventory_movements (
        id, branch_id, product_id, movement_type, quantity_delta, quantity_after,
        order_id, order_item_id, origin_event_id, created_by, created_at
      ) VALUES (
        COALESCE((p_payload->>'movement_id')::uuid, gen_random_uuid()),
        p_branch_id,
        (p_payload->>'product_id')::uuid,
        'sale',
        - (p_payload->>'quantity')::integer,
        GREATEST(0, COALESCE((
          SELECT quantity_on_hand FROM inventory_balances
          WHERE branch_id = p_branch_id AND product_id = (p_payload->>'product_id')::uuid
        ), 0) - (p_payload->>'quantity')::integer),
        (p_payload->>'order_id')::uuid,
        (p_payload->>'order_item_id')::uuid,
        p_event_id,
        (p_payload->>'added_by')::uuid,
        now()
      );
      INSERT INTO inventory_balances (branch_id, product_id, quantity_on_hand, version, updated_at)
      VALUES (
        p_branch_id,
        (p_payload->>'product_id')::uuid,
        GREATEST(0, COALESCE((SELECT quantity_on_hand FROM inventory_balances WHERE branch_id = p_branch_id AND product_id = (p_payload->>'product_id')::uuid), 0) - (p_payload->>'quantity')::integer),
        1,
        now()
      )
      ON CONFLICT (branch_id, product_id) DO UPDATE
      SET quantity_on_hand = inventory_balances.quantity_on_hand - (p_payload->>'quantity')::integer,
          version = inventory_balances.version + 1,
          updated_at = now();

    WHEN 'order.item_voided' THEN
      UPDATE order_items
      SET status = 'voided',
          voided_at = now(),
          void_reason = p_payload->>'void_reason'
      WHERE id = (p_payload->>'order_item_id')::uuid AND branch_id = p_branch_id;
      UPDATE orders
      SET product_subtotal_minor = product_subtotal_minor - (
            SELECT line_total_minor FROM order_items WHERE id = (p_payload->>'order_item_id')::uuid
          ),
          subtotal_minor = product_subtotal_minor - (
            SELECT line_total_minor FROM order_items WHERE id = (p_payload->>'order_item_id')::uuid
          ) + gaming_subtotal_minor,
          total_minor = product_subtotal_minor - (
            SELECT line_total_minor FROM order_items WHERE id = (p_payload->>'order_item_id')::uuid
          ) + gaming_subtotal_minor - discount_minor + tax_minor
      WHERE id = (p_payload->>'order_id')::uuid;
      INSERT INTO inventory_movements (
        id, branch_id, product_id, movement_type, quantity_delta, quantity_after,
        order_id, order_item_id, origin_event_id, created_by, created_at
      )
      SELECT
        gen_random_uuid(),
        p_branch_id,
        oi.product_id,
        'sale_void',
        oi.quantity,
        COALESCE(b.quantity_on_hand, 0) + oi.quantity,
        oi.order_id,
        oi.id,
        p_event_id,
        (p_payload->>'voided_by')::uuid,
        now()
      FROM order_items oi
      LEFT JOIN inventory_balances b
        ON b.branch_id = p_branch_id AND b.product_id = oi.product_id
      WHERE oi.id = (p_payload->>'order_item_id')::uuid;
      UPDATE inventory_balances
      SET quantity_on_hand = quantity_on_hand + (p_payload->>'quantity')::integer,
          version = version + 1,
          updated_at = now()
      WHERE branch_id = p_branch_id AND product_id = (
        SELECT product_id FROM order_items WHERE id = (p_payload->>'order_item_id')::uuid
      );

    WHEN 'payment.captured' THEN
      -- Sequencing/audit only. Must not close the sale or write a captured payment.
      SELECT * INTO v_order
      FROM orders
      WHERE id = (p_payload->>'order_id')::uuid
      FOR UPDATE;
      IF NOT FOUND THEN
        RAISE EXCEPTION 'order_not_found' USING ERRCODE = 'P0002';
      END IF;
      IF v_order.branch_id <> p_branch_id THEN
        RAISE EXCEPTION 'branch_mismatch' USING ERRCODE = '42501';
      END IF;
      IF v_order.status NOT IN ('open', 'checkout_pending') THEN
        RAISE EXCEPTION 'order_not_payable' USING ERRCODE = 'P0001';
      END IF;
      v_applied := COALESCE((p_payload->>'amount_applied_minor')::bigint, -1);
      v_tendered := COALESCE((p_payload->>'amount_tendered_minor')::bigint, -1);
      IF v_applied < 0 OR v_tendered < v_applied THEN
        RAISE EXCEPTION 'amount_mismatch' USING ERRCODE = '22023';
      END IF;
      UPDATE orders
      SET status = 'checkout_pending'
      WHERE id = v_order.id AND status = 'open';

    WHEN 'order.paid' THEN
      SELECT * INTO v_order
      FROM orders
      WHERE id = (p_payload->>'order_id')::uuid
      FOR UPDATE;
      IF NOT FOUND THEN
        RAISE EXCEPTION 'order_not_found' USING ERRCODE = 'P0002';
      END IF;
      IF v_order.branch_id <> p_branch_id THEN
        RAISE EXCEPTION 'branch_mismatch' USING ERRCODE = '42501';
      END IF;
      IF v_order.status <> 'checkout_pending' THEN
        RAISE EXCEPTION 'order_not_checkout_pending' USING ERRCODE = 'P0001';
      END IF;
      IF v_order.subtotal_minor + v_order.tax_minor - v_order.discount_minor <> v_order.total_minor THEN
        RAISE EXCEPTION 'total_identity' USING ERRCODE = '22023';
      END IF;

      v_snap := COALESCE(p_payload->'receipt_snapshot', '{}'::jsonb);
      -- Copy snapshot tax. Never derive tax from tax_rate_bps.
      v_snap_tax := COALESCE((v_snap->>'tax_minor')::bigint, (p_payload->>'tax_minor')::bigint, v_order.tax_minor);
      v_snap_rate := COALESCE((v_snap->>'tax_rate_bps')::integer, (p_payload->>'tax_rate_bps')::integer, v_order.tax_rate_bps);
      v_snap_sub := COALESCE((v_snap->>'subtotal_minor')::bigint, (p_payload->>'subtotal_minor')::bigint, v_order.subtotal_minor);
      v_total := COALESCE((p_payload->>'total_minor')::bigint, (v_snap->>'total_minor')::bigint, v_order.total_minor);
      v_applied := (p_payload->>'amount_applied_minor')::bigint;
      v_tendered := (p_payload->>'amount_tendered_minor')::bigint;
      v_change := (p_payload->>'change_minor')::bigint;

      IF v_snap_sub + v_snap_tax - v_order.discount_minor <> v_total THEN
        RAISE EXCEPTION 'total_identity' USING ERRCODE = '22023';
      END IF;
      IF v_applied IS NULL OR v_tendered IS NULL OR v_change IS NULL THEN
        RAISE EXCEPTION 'amount_mismatch' USING ERRCODE = '22023';
      END IF;
      IF v_applied <> v_total OR v_tendered < v_applied OR v_change <> (v_tendered - v_applied) THEN
        RAISE EXCEPTION 'amount_mismatch' USING ERRCODE = '22023';
      END IF;
      IF v_order.receipt_snapshot IS NOT NULL THEN
        RAISE EXCEPTION 'receipt_already_stored' USING ERRCODE = 'P0001';
      END IF;

      INSERT INTO payments (
        id, branch_id, order_id, payment_method_id, payment_type,
        amount_due_minor, amount_tendered_minor, amount_applied_minor, change_minor,
        status, cashier_id, paid_at, origin_event_id
      ) VALUES (
        (p_payload->>'payment_id')::uuid,
        p_branch_id,
        v_order.id,
        COALESCE((p_payload->>'payment_method_id')::uuid, '11111111-1111-1111-1111-111111111111'),
        'sale',
        COALESCE((p_payload->>'amount_due_minor')::bigint, v_total),
        v_tendered,
        v_applied,
        v_change,
        'captured',
        COALESCE((p_payload->>'closed_by')::uuid, (p_payload->>'cashier_id')::uuid),
        COALESCE((p_payload->>'closed_at')::timestamptz, (p_payload->>'paid_at')::timestamptz, now()),
        p_event_id
      )
      ON CONFLICT (id) DO NOTHING;

      IF (
        SELECT COUNT(*) FROM payments
        WHERE order_id = v_order.id AND payment_type = 'sale' AND status = 'captured'
      ) <> 1 THEN
        RAISE EXCEPTION 'duplicate_captured_sale' USING ERRCODE = 'P0001';
      END IF;

      UPDATE orders
      SET status = 'paid',
          amount_paid_minor = v_applied,
          change_minor = v_change,
          tax_minor = v_snap_tax,
          tax_rate_bps = v_snap_rate,
          subtotal_minor = v_snap_sub,
          total_minor = v_total,
          receipt_number = p_payload->>'receipt_number',
          receipt_snapshot = v_snap,
          closed_by = COALESCE((p_payload->>'closed_by')::uuid, (p_payload->>'cashier_id')::uuid),
          closed_at = COALESCE((p_payload->>'closed_at')::timestamptz, (p_payload->>'paid_at')::timestamptz, now())
      WHERE id = v_order.id AND branch_id = p_branch_id AND status = 'checkout_pending';

      IF NOT FOUND THEN
        RAISE EXCEPTION 'order_close_failed' USING ERRCODE = 'P0001';
      END IF;

    WHEN 'payment.reversed' THEN
      SELECT * INTO v_order
      FROM orders
      WHERE id = (p_payload->>'order_id')::uuid
      FOR UPDATE;
      IF NOT FOUND THEN
        RAISE EXCEPTION 'order_not_found' USING ERRCODE = 'P0002';
      END IF;
      IF v_order.branch_id <> p_branch_id THEN
        RAISE EXCEPTION 'branch_mismatch' USING ERRCODE = '42501';
      END IF;
      IF v_order.status <> 'paid' THEN
        RAISE EXCEPTION 'order_not_paid' USING ERRCODE = 'P0001';
      END IF;
      UPDATE payments
      SET status = 'reversed'
      WHERE id = (p_payload->>'parent_payment_id')::uuid
        AND branch_id = p_branch_id
        AND payment_type = 'sale'
        AND status = 'captured';
      INSERT INTO payments (
        id, branch_id, order_id, payment_method_id, payment_type,
        amount_due_minor, amount_tendered_minor, amount_applied_minor, change_minor,
        status, parent_payment_id, cashier_id, paid_at, origin_event_id, reference
      )
      SELECT
        (p_payload->>'payment_id')::uuid,
        p_branch_id,
        (p_payload->>'order_id')::uuid,
        p.payment_method_id,
        'reversal',
        p.amount_due_minor,
        0,
        p.amount_applied_minor,
        0,
        'reversed',
        p.id,
        (p_payload->>'reversed_by')::uuid,
        now(),
        p_event_id,
        p_payload->>'reason'
      FROM payments p
      WHERE p.id = (p_payload->>'parent_payment_id')::uuid;
      -- The reversal retires the receipt. order.paid keeps refusing to overwrite
      -- a stored receipt, so leaving these set made every repay after a reversal
      -- fail forever with receipt_already_stored while the till showed it paid.
      UPDATE orders
      SET status = 'checkout_pending',
          amount_paid_minor = 0,
          change_minor = 0,
          receipt_number = NULL,
          receipt_snapshot = NULL,
          closed_by = NULL,
          closed_at = NULL
      WHERE id = (p_payload->>'order_id')::uuid AND branch_id = p_branch_id;

    WHEN 'order.voided' THEN
      SELECT * INTO v_order
      FROM orders
      WHERE id = (p_payload->>'order_id')::uuid AND branch_id = p_branch_id
      FOR UPDATE;
      IF NOT FOUND THEN
        RAISE EXCEPTION 'order_not_found' USING ERRCODE = 'P0002';
      END IF;
      -- Only the open -> void transition returns stock, so a replayed event
      -- cannot credit the same lines twice.
      IF v_order.status IN ('open', 'checkout_pending') THEN
        -- A voided ticket sold nothing, so its lines go back on the shelf.
        INSERT INTO inventory_movements (
          id, branch_id, product_id, movement_type, quantity_delta, quantity_after,
          order_id, order_item_id, origin_event_id, created_by, created_at
        )
        SELECT
          gen_random_uuid(),
          p_branch_id,
          oi.product_id,
          'sale_void',
          oi.quantity,
          -- Running balance, so two lines of one product do not both claim the
          -- same closing quantity.
          COALESCE(b.quantity_on_hand, 0) + SUM(oi.quantity) OVER (
            PARTITION BY oi.product_id ORDER BY oi.id ROWS UNBOUNDED PRECEDING
          ),
          oi.order_id,
          oi.id,
          p_event_id,
          COALESCE((p_payload->>'voided_by')::uuid, v_order.opened_by),
          now()
        FROM order_items oi
        LEFT JOIN inventory_balances b
          ON b.branch_id = p_branch_id AND b.product_id = oi.product_id
        WHERE oi.order_id = v_order.id AND oi.status = 'active';

        UPDATE inventory_balances b
        SET quantity_on_hand = b.quantity_on_hand + v.quantity,
            version = b.version + 1,
            updated_at = now()
        FROM (
          SELECT oi.product_id, SUM(oi.quantity) AS quantity
          FROM order_items oi
          WHERE oi.order_id = v_order.id AND oi.status = 'active'
          GROUP BY oi.product_id
        ) v
        WHERE b.branch_id = p_branch_id AND b.product_id = v.product_id;

        UPDATE order_items
        SET status = 'voided',
            voided_at = now(),
            void_reason = COALESCE(p_payload->>'reason', 'order voided')
        WHERE order_id = v_order.id AND status = 'active';

        UPDATE orders
        SET status = 'void',
            product_subtotal_minor = 0,
            subtotal_minor = gaming_subtotal_minor,
            total_minor = gaming_subtotal_minor - discount_minor + tax_minor
        WHERE id = v_order.id;
      END IF;

    WHEN 'inventory.adjusted' THEN
      INSERT INTO inventory_movements (
        id, branch_id, product_id, movement_type, quantity_delta, quantity_after,
        reason, origin_event_id, created_by, created_at
      ) VALUES (
        (p_payload->>'movement_id')::uuid,
        p_branch_id,
        (p_payload->>'product_id')::uuid,
        p_payload->>'movement_type',
        (p_payload->>'quantity_delta')::integer,
        (p_payload->>'quantity_after')::integer,
        p_payload->>'reason',
        p_event_id,
        (p_payload->>'created_by')::uuid,
        now()
      );
      INSERT INTO inventory_balances (branch_id, product_id, quantity_on_hand, version, updated_at)
      VALUES (
        p_branch_id,
        (p_payload->>'product_id')::uuid,
        (p_payload->>'quantity_after')::integer,
        1,
        now()
      )
      ON CONFLICT (branch_id, product_id) DO UPDATE
      SET quantity_on_hand = (p_payload->>'quantity_after')::integer,
          version = inventory_balances.version + 1,
          updated_at = now();

    WHEN 'receipt.reprinted' THEN
      INSERT INTO audit_logs (
        id, branch_id, user_id, device_id, action, entity_type, entity_id, created_at, origin_event_id
      ) VALUES (
        gen_random_uuid(),
        p_branch_id,
        (p_payload->>'reprinted_by')::uuid,
        p_device_id,
        'receipt.reprinted',
        'order',
        (p_payload->>'order_id')::uuid,
        now(),
        p_event_id
      );

    ELSE
      RAISE EXCEPTION 'unknown_event_type %', p_event_type USING ERRCODE = '22023';
  END CASE;

  INSERT INTO audit_logs (
    id, branch_id, user_id, device_id, action, entity_type, entity_id, new_data, created_at, origin_event_id
  ) VALUES (
    gen_random_uuid(),
    p_branch_id,
    auth.uid(),
    p_device_id,
    p_event_type,
    split_part(p_event_type, '.', 1),
    COALESCE(
      (p_payload->>'order_id')::uuid,
      (p_payload->>'session_id')::uuid,
      (p_payload->>'payment_id')::uuid
    ),
    p_payload,
    now(),
    p_event_id
  );

  UPDATE device_sequence_cloud
  SET last_applied_sequence = p_local_sequence
  WHERE device_id = p_device_id;

  INSERT INTO sync_receipts (
    event_id, branch_id, device_id, local_sequence, event_type, payload_hash
  ) VALUES (
    p_event_id, p_branch_id, p_device_id, p_local_sequence, p_event_type, p_payload_hash
  );

  UPDATE devices SET last_seen_at = now() WHERE id = p_device_id;

  RETURN jsonb_build_object(
    'status', 'applied',
    'event_id', p_event_id,
    'local_sequence', p_local_sequence
  );
END;
$$;

REVOKE ALL ON FUNCTION public.apply_domain_event(uuid, uuid, uuid, bigint, text, jsonb, text) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION public.apply_domain_event(uuid, uuid, uuid, bigint, text, jsonb, text) TO authenticated;
