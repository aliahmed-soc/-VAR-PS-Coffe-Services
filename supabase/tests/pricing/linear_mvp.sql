-- MVP linear-only pricing: stepped rules and cloud import of stepped snapshots are rejected.

DO $$
DECLARE
  v_branch uuid := 'a1111111-1111-4111-8111-111111111111';
  v_device uuid := 'a2222222-2222-4222-8222-222222222222';
  v_user uuid := 'a3333333-3333-4333-8333-333333333333';
  v_station uuid := 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa';
  v_order uuid := 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb';
  v_session uuid := 'cccccccc-cccc-4ccc-8ccc-cccccccccccc';
  v_rule uuid := 'dddddddd-dddd-4ddd-8ddd-dddddddddddd';
  v_result jsonb;
BEGIN
  INSERT INTO auth.users (id, email) VALUES (v_user, 'pricing@local') ON CONFLICT DO NOTHING;
  INSERT INTO branches (id, code, name) VALUES (v_branch, 'PX', 'Pricing branch') ON CONFLICT (id) DO NOTHING;
  INSERT INTO user_profiles (user_id, display_name, is_system_admin, is_active)
  VALUES (v_user, 'Cashier', true, true)
  ON CONFLICT (user_id) DO UPDATE SET is_system_admin = true;
  INSERT INTO user_branch_roles (user_id, branch_id, role, is_active)
  VALUES (v_user, v_branch, 'admin', true)
  ON CONFLICT (user_id, branch_id) DO NOTHING;
  INSERT INTO devices (id, branch_id, name, device_key, is_active)
  VALUES (v_device, v_branch, 'Cashier 1', 'dev-key-b1', true)
  ON CONFLICT (id) DO NOTHING;
  INSERT INTO stations (id, branch_id, code, display_name, sort_order, is_active)
  VALUES (v_station, v_branch, 'PS1', 'PS1', 1, true)
  ON CONFLICT (id) DO NOTHING;
  PERFORM set_config('request.jwt.claim.sub', v_user::text, true);

  INSERT INTO pricing_rules (
    id, branch_id, name, rule_type, rate_minor_per_hour, effective_from
  ) VALUES (
    v_rule, v_branch, 'Linear 30', 'linear', 3000, now()
  ) ON CONFLICT (id) DO NOTHING;

  BEGIN
    INSERT INTO pricing_rules (
      id, branch_id, name, rule_type, rate_minor_per_hour, effective_from
    ) VALUES (
      'eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee', v_branch, 'Stepped', 'stepped', 3000, now()
    );
    RAISE EXCEPTION 'postgres must reject stepped pricing_rules';
  EXCEPTION
    WHEN check_violation THEN
      NULL;
  END;

  BEGIN
    INSERT INTO pricing_rules (
      id, branch_id, name, rule_type, rate_minor_per_hour, effective_from
    ) VALUES (
      'ffffffff-ffff-4fff-8fff-ffffffffffff', v_branch, 'Neg', 'linear', -1, now()
    );
    RAISE EXCEPTION 'postgres must reject negative rates';
  EXCEPTION
    WHEN check_violation THEN
      NULL;
  END;

  INSERT INTO orders (
    id, branch_id, order_type, status, currency_code, opened_by, opened_at,
    tax_minor, tax_rate_bps, origin_device_id
  ) VALUES (
    v_order, v_branch, 'gaming', 'open', 'EGP', v_user, now(), 0, 0, v_device
  ) ON CONFLICT (id) DO NOTHING;

  BEGIN
    PERFORM apply_domain_event(
      '99999999-9999-4999-8999-999999999991',
      v_branch,
      v_device,
      1,
      'session.started',
      jsonb_build_object(
        'session_id', v_session,
        'station_id', v_station,
        'order_id', v_order,
        'started_at', now(),
        'pricing_rule_id', v_rule,
        'started_by', v_user,
        'pricing_snapshot', jsonb_build_object(
          'rule_type', 'stepped',
          'rate_minor_per_hour', 3000,
          'base_duration_seconds', 3600,
          'base_charge_minor', 3000
        )
      ),
      'deadbeef'
    );
    RAISE EXCEPTION 'cloud apply must not accept a stepped snapshot';
  EXCEPTION
    WHEN others THEN
      IF SQLERRM NOT LIKE '%mvp_linear_pricing_required%' THEN
        RAISE;
      END IF;
  END;

  v_result := apply_domain_event(
    '99999999-9999-4999-8999-999999999992',
    v_branch,
    v_device,
    1,
    'session.started',
    jsonb_build_object(
      'session_id', v_session,
      'station_id', v_station,
      'order_id', v_order,
      'started_at', now(),
      'pricing_rule_id', v_rule,
      'started_by', v_user,
      'pricing_snapshot', jsonb_build_object(
        'rule_type', 'linear',
        'rate_minor_per_hour', 3000
      )
    ),
    'cafebabe'
  );
  IF v_result->>'status' IS DISTINCT FROM 'applied'
     AND v_result->>'status' IS DISTINCT FROM 'already_processed' THEN
    RAISE EXCEPTION 'linear snapshot must apply, got %', v_result;
  END IF;
END $$;
