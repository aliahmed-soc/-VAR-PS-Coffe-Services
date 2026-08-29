-- RLS: cashiers see only their branch; system admins see all; anonymous denied.

CREATE OR REPLACE FUNCTION public.is_system_admin()
RETURNS boolean
LANGUAGE sql
STABLE
SECURITY DEFINER
SET search_path = public
AS $$
  SELECT COALESCE((
    SELECT is_system_admin AND is_active
    FROM user_profiles
    WHERE user_id = auth.uid()
  ), false);
$$;

CREATE OR REPLACE FUNCTION public.has_branch_access(p_branch_id uuid)
RETURNS boolean
LANGUAGE sql
STABLE
SECURITY DEFINER
SET search_path = public
AS $$
  SELECT public.is_system_admin()
     OR EXISTS (
       SELECT 1
       FROM user_branch_roles r
       WHERE r.user_id = auth.uid()
         AND r.branch_id = p_branch_id
         AND r.is_active
     );
$$;

ALTER TABLE branches ENABLE ROW LEVEL SECURITY;
ALTER TABLE user_profiles ENABLE ROW LEVEL SECURITY;
ALTER TABLE user_branch_roles ENABLE ROW LEVEL SECURITY;
ALTER TABLE devices ENABLE ROW LEVEL SECURITY;
ALTER TABLE stations ENABLE ROW LEVEL SECURITY;
ALTER TABLE pricing_rules ENABLE ROW LEVEL SECURITY;
ALTER TABLE categories ENABLE ROW LEVEL SECURITY;
ALTER TABLE products ENABLE ROW LEVEL SECURITY;
ALTER TABLE branch_products ENABLE ROW LEVEL SECURITY;
ALTER TABLE inventory_balances ENABLE ROW LEVEL SECURITY;
ALTER TABLE inventory_movements ENABLE ROW LEVEL SECURITY;
ALTER TABLE orders ENABLE ROW LEVEL SECURITY;
ALTER TABLE order_items ENABLE ROW LEVEL SECURITY;
ALTER TABLE gaming_sessions ENABLE ROW LEVEL SECURITY;
ALTER TABLE payment_methods ENABLE ROW LEVEL SECURITY;
ALTER TABLE payments ENABLE ROW LEVEL SECURITY;
ALTER TABLE expenses ENABLE ROW LEVEL SECURITY;
ALTER TABLE cashier_shifts ENABLE ROW LEVEL SECURITY;
ALTER TABLE audit_logs ENABLE ROW LEVEL SECURITY;
ALTER TABLE app_settings ENABLE ROW LEVEL SECURITY;
ALTER TABLE sync_receipts ENABLE ROW LEVEL SECURITY;
ALTER TABLE device_sequence_cloud ENABLE ROW LEVEL SECURITY;

-- Authenticated users can read their own profile; admins read all.
CREATE POLICY user_profiles_select ON user_profiles
  FOR SELECT TO authenticated
  USING (user_id = auth.uid() OR public.is_system_admin());

CREATE POLICY user_profiles_admin_write ON user_profiles
  FOR ALL TO authenticated
  USING (public.is_system_admin())
  WITH CHECK (public.is_system_admin());

CREATE POLICY branches_select ON branches
  FOR SELECT TO authenticated
  USING (public.has_branch_access(id) OR public.is_system_admin());

CREATE POLICY branches_admin_write ON branches
  FOR ALL TO authenticated
  USING (public.is_system_admin())
  WITH CHECK (public.is_system_admin());

CREATE POLICY roles_select ON user_branch_roles
  FOR SELECT TO authenticated
  USING (user_id = auth.uid() OR public.is_system_admin());

CREATE POLICY roles_admin_write ON user_branch_roles
  FOR ALL TO authenticated
  USING (public.is_system_admin())
  WITH CHECK (public.is_system_admin());

CREATE POLICY devices_select ON devices
  FOR SELECT TO authenticated
  USING (public.has_branch_access(branch_id));

CREATE POLICY devices_admin_write ON devices
  FOR ALL TO authenticated
  USING (public.is_system_admin())
  WITH CHECK (public.is_system_admin());

CREATE POLICY stations_select ON stations
  FOR SELECT TO authenticated
  USING (public.has_branch_access(branch_id));

CREATE POLICY stations_admin_write ON stations
  FOR ALL TO authenticated
  USING (public.is_system_admin())
  WITH CHECK (public.is_system_admin());

CREATE POLICY pricing_select ON pricing_rules
  FOR SELECT TO authenticated
  USING (public.has_branch_access(branch_id));

CREATE POLICY pricing_admin_write ON pricing_rules
  FOR ALL TO authenticated
  USING (public.is_system_admin())
  WITH CHECK (public.is_system_admin());

CREATE POLICY categories_select ON categories
  FOR SELECT TO authenticated
  USING (auth.uid() IS NOT NULL);

CREATE POLICY categories_admin_write ON categories
  FOR ALL TO authenticated
  USING (public.is_system_admin())
  WITH CHECK (public.is_system_admin());

CREATE POLICY products_select ON products
  FOR SELECT TO authenticated
  USING (auth.uid() IS NOT NULL);

CREATE POLICY products_admin_write ON products
  FOR ALL TO authenticated
  USING (public.is_system_admin())
  WITH CHECK (public.is_system_admin());

CREATE POLICY branch_products_select ON branch_products
  FOR SELECT TO authenticated
  USING (public.has_branch_access(branch_id));

CREATE POLICY branch_products_admin_write ON branch_products
  FOR ALL TO authenticated
  USING (public.is_system_admin())
  WITH CHECK (public.is_system_admin());

CREATE POLICY inventory_balances_select ON inventory_balances
  FOR SELECT TO authenticated
  USING (public.has_branch_access(branch_id));

-- Operational writes go through apply_domain_event (SECURITY DEFINER).
-- Direct client writes on financial/inventory tables are denied.

CREATE POLICY inventory_movements_select ON inventory_movements
  FOR SELECT TO authenticated
  USING (public.has_branch_access(branch_id));

CREATE POLICY orders_select ON orders
  FOR SELECT TO authenticated
  USING (public.has_branch_access(branch_id));

CREATE POLICY order_items_select ON order_items
  FOR SELECT TO authenticated
  USING (public.has_branch_access(branch_id));

CREATE POLICY sessions_select ON gaming_sessions
  FOR SELECT TO authenticated
  USING (public.has_branch_access(branch_id));

CREATE POLICY payments_select ON payments
  FOR SELECT TO authenticated
  USING (public.has_branch_access(branch_id));

CREATE POLICY payment_methods_select ON payment_methods
  FOR SELECT TO authenticated
  USING (auth.uid() IS NOT NULL);

CREATE POLICY expenses_select ON expenses
  FOR SELECT TO authenticated
  USING (public.has_branch_access(branch_id));

CREATE POLICY shifts_select ON cashier_shifts
  FOR SELECT TO authenticated
  USING (public.has_branch_access(branch_id));

CREATE POLICY audit_select ON audit_logs
  FOR SELECT TO authenticated
  USING (public.has_branch_access(branch_id) OR public.is_system_admin());

CREATE POLICY settings_select ON app_settings
  FOR SELECT TO authenticated
  USING (
    public.is_system_admin()
    OR branch_id IS NULL
    OR public.has_branch_access(branch_id)
  );

CREATE POLICY settings_admin_write ON app_settings
  FOR ALL TO authenticated
  USING (public.is_system_admin())
  WITH CHECK (public.is_system_admin());

CREATE POLICY sync_receipts_select ON sync_receipts
  FOR SELECT TO authenticated
  USING (public.has_branch_access(branch_id) OR public.is_system_admin());

CREATE POLICY device_seq_select ON device_sequence_cloud
  FOR SELECT TO authenticated
  USING (public.is_system_admin());

REVOKE ALL ON TABLE payments FROM anon, authenticated;
REVOKE ALL ON TABLE inventory_movements FROM anon, authenticated;
REVOKE ALL ON TABLE gaming_sessions FROM anon, authenticated;
REVOKE ALL ON TABLE orders FROM anon, authenticated;
REVOKE ALL ON TABLE order_items FROM anon, authenticated;
REVOKE ALL ON TABLE inventory_balances FROM anon, authenticated;
REVOKE ALL ON TABLE sync_receipts FROM anon, authenticated;
REVOKE ALL ON TABLE device_sequence_cloud FROM anon, authenticated;

GRANT SELECT ON TABLE payments TO authenticated;
GRANT SELECT ON TABLE inventory_movements TO authenticated;
GRANT SELECT ON TABLE gaming_sessions TO authenticated;
GRANT SELECT ON TABLE orders TO authenticated;
GRANT SELECT ON TABLE order_items TO authenticated;
GRANT SELECT ON TABLE inventory_balances TO authenticated;
GRANT SELECT ON TABLE sync_receipts TO authenticated;
GRANT SELECT ON TABLE branches TO authenticated;
GRANT SELECT ON TABLE stations TO authenticated;
GRANT SELECT ON TABLE products TO authenticated;
GRANT SELECT ON TABLE categories TO authenticated;
GRANT SELECT ON TABLE branch_products TO authenticated;
GRANT SELECT ON TABLE pricing_rules TO authenticated;
GRANT SELECT ON TABLE payment_methods TO authenticated;
GRANT SELECT ON TABLE user_profiles TO authenticated;
GRANT SELECT ON TABLE user_branch_roles TO authenticated;
GRANT SELECT ON TABLE devices TO authenticated;
GRANT SELECT ON TABLE audit_logs TO authenticated;
GRANT SELECT ON TABLE app_settings TO authenticated;
GRANT SELECT ON TABLE expenses TO authenticated;
GRANT SELECT ON TABLE cashier_shifts TO authenticated;
