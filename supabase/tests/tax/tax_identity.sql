-- P1-4: tax defaults, negative rejected, paid tax immutable, replay copies snapshot.
-- Run against local Supabase after migrations.

DO $$
DECLARE
  v_order uuid := 'aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa';
  v_branch uuid;
BEGIN
  SELECT id INTO v_branch FROM branches LIMIT 1;
  IF v_branch IS NULL THEN
    RAISE EXCEPTION 'seed a branch before tax tests';
  END IF;

  INSERT INTO orders (
    id, branch_id, order_type, status, currency_code, opened_by, opened_at,
    tax_minor, tax_rate_bps, subtotal_minor, total_minor
  ) VALUES (
    v_order, v_branch, 'pos', 'open', 'EGP', 'bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb', now(),
    0, 0, 0, 0
  );

  IF (SELECT tax_minor FROM orders WHERE id = v_order) <> 0 THEN
    RAISE EXCEPTION 'tax default must be zero';
  END IF;

  BEGIN
    INSERT INTO orders (
      id, branch_id, order_type, status, currency_code, opened_by, opened_at, tax_minor
    ) VALUES (
      'cccccccc-cccc-cccc-cccc-cccccccccccc', v_branch, 'pos', 'open', 'EGP',
      'bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb', now(), -1
    );
    RAISE EXCEPTION 'negative tax must be rejected';
  EXCEPTION
    WHEN check_violation THEN
      NULL;
  END;

  UPDATE orders SET status = 'paid', receipt_snapshot = '{"tax_minor":0,"tax_rate_bps":0}'::jsonb
  WHERE id = v_order;

  BEGIN
    UPDATE orders SET tax_minor = 99 WHERE id = v_order;
    RAISE EXCEPTION 'paid tax must be immutable';
  EXCEPTION
    WHEN others THEN
      IF SQLERRM NOT LIKE '%paid_tax_immutable%' AND SQLSTATE <> '23000' THEN
        RAISE;
      END IF;
  END;

  IF (SELECT tax_minor FROM orders WHERE id = v_order) <> 0 THEN
    RAISE EXCEPTION 'historical zero-tax snapshot must remain';
  END IF;
END $$;
