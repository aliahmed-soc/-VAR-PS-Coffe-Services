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

    WHEN 'order.paid', 'payment.captured' THEN
      INSERT INTO payments (
        id, branch_id, order_id, payment_method_id, payment_type,
        amount_due_minor, amount_tendered_minor, amount_applied_minor, change_minor,
        status, cashier_id, paid_at, origin_event_id
      ) VALUES (
        (p_payload->>'payment_id')::uuid,
        p_branch_id,
        (p_payload->>'order_id')::uuid,
        COALESCE((p_payload->>'payment_method_id')::uuid, '11111111-1111-1111-1111-111111111111'),
        'sale',
        (p_payload->>'amount_due_minor')::bigint,
        (p_payload->>'amount_tendered_minor')::bigint,
        (p_payload->>'amount_applied_minor')::bigint,
        (p_payload->>'change_minor')::bigint,
        'captured',
        (p_payload->>'cashier_id')::uuid,
        (p_payload->>'paid_at')::timestamptz,
        p_event_id
      )
      ON CONFLICT DO NOTHING;
      -- Replay copies receipt-snapshot tax. Never derive tax from tax_rate_bps.
      UPDATE orders
      SET status = 'paid',
          amount_paid_minor = (p_payload->>'amount_applied_minor')::bigint,
          change_minor = (p_payload->>'change_minor')::bigint,
          tax_minor = COALESCE((p_payload->'receipt_snapshot'->>'tax_minor')::bigint, (p_payload->>'tax_minor')::bigint, tax_minor),
          tax_rate_bps = COALESCE((p_payload->'receipt_snapshot'->>'tax_rate_bps')::integer, (p_payload->>'tax_rate_bps')::integer, tax_rate_bps),
          subtotal_minor = COALESCE((p_payload->'receipt_snapshot'->>'subtotal_minor')::bigint, (p_payload->>'subtotal_minor')::bigint, subtotal_minor),
          total_minor = COALESCE((p_payload->>'total_minor')::bigint, (p_payload->'receipt_snapshot'->>'total_minor')::bigint, total_minor),
          receipt_number = COALESCE(p_payload->>'receipt_number', receipt_number),
          receipt_snapshot = COALESCE(p_payload->'receipt_snapshot', receipt_snapshot),
          closed_by = COALESCE((p_payload->>'closed_by')::uuid, (p_payload->>'cashier_id')::uuid),
          closed_at = (p_payload->>'closed_at')::timestamptz
      WHERE id = (p_payload->>'order_id')::uuid AND branch_id = p_branch_id;

    WHEN 'payment.reversed' THEN
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
      UPDATE orders
      SET status = 'checkout_pending',
          amount_paid_minor = 0,
          change_minor = 0,
          closed_by = NULL,
          closed_at = NULL
      WHERE id = (p_payload->>'order_id')::uuid AND branch_id = p_branch_id;

    WHEN 'order.voided' THEN
      UPDATE orders
      SET status = 'void'
      WHERE id = (p_payload->>'order_id')::uuid
        AND branch_id = p_branch_id
        AND status IN ('open', 'checkout_pending');

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

CREATE OR REPLACE FUNCTION public.pull_branch_since(
  p_branch_id uuid,
  p_after timestamptz
)
RETURNS jsonb
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public
AS $$
BEGIN
  IF auth.uid() IS NULL THEN
    RAISE EXCEPTION 'not_authenticated' USING ERRCODE = '28000';
  END IF;
  IF NOT public.has_branch_access(p_branch_id) THEN
    RAISE EXCEPTION 'branch_forbidden' USING ERRCODE = '42501';
  END IF;

  RETURN jsonb_build_object(
    'orders', COALESCE((SELECT jsonb_agg(to_jsonb(o)) FROM orders o WHERE o.branch_id = p_branch_id AND o.opened_at > p_after), '[]'::jsonb),
    'payments', COALESCE((SELECT jsonb_agg(to_jsonb(p)) FROM payments p WHERE p.branch_id = p_branch_id AND p.paid_at > p_after), '[]'::jsonb),
    'sessions', COALESCE((SELECT jsonb_agg(to_jsonb(s)) FROM gaming_sessions s WHERE s.branch_id = p_branch_id AND s.started_at > p_after), '[]'::jsonb),
    'inventory_balances', COALESCE((SELECT jsonb_agg(to_jsonb(b)) FROM inventory_balances b WHERE b.branch_id = p_branch_id), '[]'::jsonb),
    'sync_receipts', COALESCE((
      SELECT jsonb_agg(to_jsonb(r)) FROM sync_receipts r
      WHERE r.branch_id = p_branch_id AND r.processed_at > p_after
    ), '[]'::jsonb)
  );
END;
$$;

REVOKE ALL ON FUNCTION public.pull_branch_since(uuid, timestamptz) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION public.pull_branch_since(uuid, timestamptz) TO authenticated;
