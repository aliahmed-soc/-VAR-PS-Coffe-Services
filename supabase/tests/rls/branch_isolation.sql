-- RLS allow/deny matrix for vanilla Postgres CI (auth stub + SET ROLE).

CREATE OR REPLACE FUNCTION public.test_rls_branch_isolation()
RETURNS void
LANGUAGE plpgsql
AS $$
DECLARE
  v_b1 uuid := '11111111-1111-4111-8111-111111111111';
  v_b2 uuid := 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa';
  v_admin uuid := '33333333-3333-4333-8333-333333333333';
  v_c1 uuid := 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb';
  v_seen int;
BEGIN
  INSERT INTO auth.users (id, email) VALUES
    (v_admin, 'admin@local'),
    (v_c1, 'c1@local')
  ON CONFLICT DO NOTHING;
  INSERT INTO branches (id, code, name) VALUES
    (v_b1, 'B1', 'Branch 1'),
    (v_b2, 'B2', 'Branch 2')
  ON CONFLICT (id) DO NOTHING;
  INSERT INTO user_profiles (user_id, display_name, is_system_admin, is_active) VALUES
    (v_admin, 'Admin', true, true),
    (v_c1, 'Cashier', false, true)
  ON CONFLICT (user_id) DO UPDATE SET is_system_admin = EXCLUDED.is_system_admin;
  INSERT INTO user_branch_roles (user_id, branch_id, role, is_active) VALUES
    (v_admin, v_b1, 'admin', true),
    (v_c1, v_b1, 'cashier', true)
  ON CONFLICT (user_id, branch_id) DO NOTHING;
  INSERT INTO orders (
    id, branch_id, order_type, status, currency_code, opened_by, opened_at
  ) VALUES
    ('cccccccc-cccc-4ccc-8ccc-cccccccccccc', v_b1, 'pos', 'open', 'EGP', v_c1, now()),
    ('dddddddd-dddd-4ddd-8ddd-dddddddddddd', v_b2, 'pos', 'open', 'EGP', v_admin, now())
  ON CONFLICT (id) DO NOTHING;

  GRANT USAGE ON SCHEMA public TO authenticated, anon;
  GRANT SELECT ON ALL TABLES IN SCHEMA public TO authenticated, anon;

  PERFORM set_config('request.jwt.claim.sub', v_c1::text, true);
  SET LOCAL ROLE authenticated;
  SELECT COUNT(*) INTO v_seen FROM orders WHERE branch_id = v_b2;
  IF v_seen <> 0 THEN
    RAISE EXCEPTION 'cashier must not see the other branch';
  END IF;
  SELECT COUNT(*) INTO v_seen FROM orders WHERE branch_id = v_b1;
  IF v_seen < 1 THEN
    RAISE EXCEPTION 'cashier must see their own branch';
  END IF;
  RESET ROLE;

  PERFORM set_config('request.jwt.claim.sub', v_admin::text, true);
  SET LOCAL ROLE authenticated;
  SELECT COUNT(*) INTO v_seen FROM orders;
  IF v_seen < 2 THEN
    RAISE EXCEPTION 'admin must see both branches';
  END IF;
  RESET ROLE;

  SET LOCAL ROLE anon;
  SELECT COUNT(*) INTO v_seen FROM orders;
  IF v_seen <> 0 THEN
    RAISE EXCEPTION 'anon must see no orders';
  END IF;
  RESET ROLE;
END;
$$;

SELECT public.test_rls_branch_isolation();
