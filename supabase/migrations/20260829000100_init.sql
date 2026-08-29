-- PlayStation Café POS — PostgreSQL central schema

CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE TABLE branches (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  code text NOT NULL UNIQUE,
  name text NOT NULL,
  timezone text NOT NULL DEFAULT 'Africa/Cairo',
  currency_code char(3) NOT NULL DEFAULT 'EGP',
  is_active boolean NOT NULL DEFAULT true,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE user_profiles (
  user_id uuid PRIMARY KEY REFERENCES auth.users(id) ON DELETE CASCADE,
  display_name text NOT NULL,
  preferred_locale text NOT NULL DEFAULT 'en' CHECK (preferred_locale IN ('en', 'ar')),
  is_system_admin boolean NOT NULL DEFAULT false,
  is_active boolean NOT NULL DEFAULT true,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE user_branch_roles (
  user_id uuid NOT NULL REFERENCES user_profiles(user_id) ON DELETE CASCADE,
  branch_id uuid NOT NULL REFERENCES branches(id),
  role text NOT NULL CHECK (role IN ('admin', 'cashier')),
  offline_access_allowed boolean NOT NULL DEFAULT true,
  is_active boolean NOT NULL DEFAULT true,
  created_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (user_id, branch_id)
);
CREATE INDEX idx_user_branch_roles_branch ON user_branch_roles(branch_id);

CREATE TABLE devices (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  branch_id uuid NOT NULL REFERENCES branches(id),
  name text NOT NULL,
  device_key text NOT NULL UNIQUE,
  is_active boolean NOT NULL DEFAULT true,
  paired_at timestamptz NOT NULL DEFAULT now(),
  last_seen_at timestamptz,
  app_version text
);

CREATE TABLE stations (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  branch_id uuid NOT NULL REFERENCES branches(id),
  code text NOT NULL,
  display_name text NOT NULL,
  sort_order integer NOT NULL DEFAULT 0,
  is_active boolean NOT NULL DEFAULT true,
  UNIQUE (branch_id, code)
);

CREATE TABLE pricing_rules (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  branch_id uuid NOT NULL REFERENCES branches(id),
  name text NOT NULL,
  rule_type text NOT NULL CHECK (rule_type IN ('linear', 'stepped')),
  rate_minor_per_hour bigint,
  billing_increment_seconds integer,
  base_duration_seconds integer,
  base_charge_minor bigint,
  step_duration_seconds integer,
  step_charge_minor bigint,
  round_partial_step_up boolean NOT NULL DEFAULT true,
  version integer NOT NULL DEFAULT 1,
  effective_from timestamptz NOT NULL,
  retired_at timestamptz,
  created_by uuid
);

CREATE TABLE categories (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  name text NOT NULL,
  name_ar text,
  sort_order integer NOT NULL DEFAULT 0,
  is_active boolean NOT NULL DEFAULT true
);

CREATE TABLE products (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  category_id uuid NOT NULL REFERENCES categories(id),
  sku text,
  barcode text,
  name text NOT NULL,
  name_ar text,
  default_sell_price_minor bigint NOT NULL CHECK (default_sell_price_minor >= 0),
  default_cost_price_minor bigint NOT NULL CHECK (default_cost_price_minor >= 0),
  is_active boolean NOT NULL DEFAULT true,
  image_key text,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX idx_products_sku ON products(sku) WHERE sku IS NOT NULL;
CREATE UNIQUE INDEX idx_products_barcode ON products(barcode) WHERE barcode IS NOT NULL;
CREATE INDEX idx_products_name ON products(name);
CREATE INDEX idx_products_category ON products(category_id);

CREATE TABLE branch_products (
  branch_id uuid NOT NULL REFERENCES branches(id),
  product_id uuid NOT NULL REFERENCES products(id),
  sell_price_override_minor bigint,
  cost_price_override_minor bigint,
  minimum_stock integer NOT NULL DEFAULT 0,
  is_active boolean NOT NULL DEFAULT true,
  updated_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (branch_id, product_id)
);

CREATE TABLE inventory_balances (
  branch_id uuid NOT NULL REFERENCES branches(id),
  product_id uuid NOT NULL REFERENCES products(id),
  quantity_on_hand integer NOT NULL CHECK (quantity_on_hand >= 0),
  version bigint NOT NULL DEFAULT 0,
  updated_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (branch_id, product_id)
);

CREATE TABLE orders (
  id uuid PRIMARY KEY,
  branch_id uuid NOT NULL REFERENCES branches(id),
  order_type text NOT NULL CHECK (order_type IN ('gaming', 'pos')),
  status text NOT NULL CHECK (status IN ('open', 'checkout_pending', 'paid', 'void', 'refunded')),
  product_subtotal_minor bigint NOT NULL DEFAULT 0 CHECK (product_subtotal_minor >= 0),
  gaming_subtotal_minor bigint NOT NULL DEFAULT 0 CHECK (gaming_subtotal_minor >= 0),
  subtotal_minor bigint NOT NULL DEFAULT 0 CHECK (subtotal_minor >= 0),
  discount_minor bigint NOT NULL DEFAULT 0 CHECK (discount_minor >= 0),
  tax_minor bigint NOT NULL DEFAULT 0 CHECK (tax_minor >= 0),
  tax_rate_bps integer NOT NULL DEFAULT 0 CHECK (tax_rate_bps >= 0),
  total_minor bigint NOT NULL DEFAULT 0 CHECK (total_minor >= 0),
  amount_paid_minor bigint NOT NULL DEFAULT 0 CHECK (amount_paid_minor >= 0),
  change_minor bigint NOT NULL DEFAULT 0 CHECK (change_minor >= 0),
  currency_code char(3) NOT NULL DEFAULT 'EGP',
  receipt_number text,
  receipt_snapshot jsonb,
  origin_device_id uuid,
  opened_by uuid NOT NULL,
  closed_by uuid,
  opened_at timestamptz NOT NULL,
  closed_at timestamptz,
  CONSTRAINT orders_subtotal_identity CHECK (subtotal_minor = product_subtotal_minor + gaming_subtotal_minor),
  CONSTRAINT orders_total_identity CHECK (total_minor = subtotal_minor + tax_minor - discount_minor)
);
CREATE INDEX idx_orders_branch_status ON orders(branch_id, status);
CREATE INDEX idx_orders_branch_opened ON orders(branch_id, opened_at);
CREATE UNIQUE INDEX idx_orders_receipt ON orders(receipt_number) WHERE receipt_number IS NOT NULL;

CREATE OR REPLACE FUNCTION public.orders_paid_tax_immutable()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
  IF OLD.status = 'paid' AND (
    NEW.tax_minor IS DISTINCT FROM OLD.tax_minor
    OR NEW.tax_rate_bps IS DISTINCT FROM OLD.tax_rate_bps
    OR NEW.subtotal_minor IS DISTINCT FROM OLD.subtotal_minor
  ) THEN
    RAISE EXCEPTION 'paid_tax_immutable' USING ERRCODE = '23000';
  END IF;
  RETURN NEW;
END;
$$;

CREATE TRIGGER orders_paid_tax_immutable
BEFORE UPDATE ON orders
FOR EACH ROW
EXECUTE PROCEDURE public.orders_paid_tax_immutable();

CREATE TABLE order_items (
  id uuid PRIMARY KEY,
  branch_id uuid NOT NULL REFERENCES branches(id),
  order_id uuid NOT NULL REFERENCES orders(id),
  product_id uuid NOT NULL REFERENCES products(id),
  product_name_snapshot text NOT NULL,
  quantity integer NOT NULL CHECK (quantity > 0),
  unit_price_minor bigint NOT NULL CHECK (unit_price_minor >= 0),
  unit_cost_minor bigint NOT NULL CHECK (unit_cost_minor >= 0),
  line_total_minor bigint NOT NULL CHECK (line_total_minor >= 0),
  status text NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'voided')),
  added_by uuid NOT NULL,
  added_at timestamptz NOT NULL,
  voided_at timestamptz,
  void_reason text
);
CREATE INDEX idx_order_items_order ON order_items(order_id);

CREATE TABLE gaming_sessions (
  id uuid PRIMARY KEY,
  branch_id uuid NOT NULL REFERENCES branches(id),
  station_id uuid NOT NULL REFERENCES stations(id),
  order_id uuid NOT NULL UNIQUE REFERENCES orders(id),
  status text NOT NULL CHECK (status IN ('active', 'stopped', 'void')),
  started_at timestamptz NOT NULL,
  ended_at timestamptz,
  duration_seconds bigint,
  pricing_rule_id uuid REFERENCES pricing_rules(id),
  pricing_snapshot jsonb NOT NULL,
  calculated_charge_minor bigint,
  final_charge_minor bigint,
  started_by uuid NOT NULL,
  stopped_by uuid,
  clock_anomaly boolean NOT NULL DEFAULT false
);
CREATE UNIQUE INDEX idx_gaming_one_active_station
  ON gaming_sessions(branch_id, station_id) WHERE status = 'active';

CREATE TABLE inventory_movements (
  id uuid PRIMARY KEY,
  branch_id uuid NOT NULL REFERENCES branches(id),
  product_id uuid NOT NULL REFERENCES products(id),
  movement_type text NOT NULL CHECK (movement_type IN (
    'opening', 'sale', 'sale_void', 'adjustment_in', 'adjustment_out',
    'damaged', 'expired', 'refund', 'transfer_in', 'transfer_out'
  )),
  quantity_delta integer NOT NULL,
  quantity_after integer NOT NULL CHECK (quantity_after >= 0),
  order_id uuid,
  order_item_id uuid,
  reason text,
  origin_event_id uuid NOT NULL UNIQUE,
  created_by uuid NOT NULL,
  created_at timestamptz NOT NULL
);
CREATE INDEX idx_inv_movements_product ON inventory_movements(branch_id, product_id, created_at);
CREATE INDEX idx_inv_movements_order ON inventory_movements(order_id);

CREATE TABLE payment_methods (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  code text NOT NULL UNIQUE,
  name text NOT NULL,
  name_ar text,
  is_active boolean NOT NULL DEFAULT true,
  requires_reference boolean NOT NULL DEFAULT false,
  sort_order integer NOT NULL DEFAULT 0
);

CREATE TABLE payments (
  id uuid PRIMARY KEY,
  branch_id uuid NOT NULL REFERENCES branches(id),
  order_id uuid NOT NULL REFERENCES orders(id),
  payment_method_id uuid NOT NULL REFERENCES payment_methods(id),
  payment_type text NOT NULL CHECK (payment_type IN ('sale', 'refund', 'reversal')),
  amount_due_minor bigint NOT NULL CHECK (amount_due_minor >= 0),
  amount_tendered_minor bigint NOT NULL CHECK (amount_tendered_minor >= 0),
  amount_applied_minor bigint NOT NULL CHECK (amount_applied_minor >= 0),
  change_minor bigint NOT NULL CHECK (change_minor >= 0),
  status text NOT NULL CHECK (status IN ('captured', 'voided', 'refunded', 'reversed')),
  parent_payment_id uuid,
  reference text,
  cashier_id uuid NOT NULL,
  paid_at timestamptz NOT NULL,
  origin_event_id uuid NOT NULL UNIQUE
);
CREATE UNIQUE INDEX idx_payments_one_captured_sale
  ON payments(order_id) WHERE payment_type = 'sale' AND status = 'captured';

CREATE TABLE expenses (
  id uuid PRIMARY KEY,
  branch_id uuid NOT NULL REFERENCES branches(id),
  category text NOT NULL,
  amount_minor bigint NOT NULL CHECK (amount_minor >= 0),
  note text,
  expense_at timestamptz NOT NULL,
  created_by uuid NOT NULL,
  origin_event_id uuid UNIQUE
);
CREATE INDEX idx_expenses_branch ON expenses(branch_id, expense_at);

CREATE TABLE cashier_shifts (
  id uuid PRIMARY KEY,
  branch_id uuid NOT NULL REFERENCES branches(id),
  user_id uuid NOT NULL,
  device_id uuid NOT NULL,
  status text NOT NULL,
  opening_cash_minor bigint NOT NULL,
  expected_cash_minor bigint,
  closing_cash_minor bigint,
  started_at timestamptz NOT NULL,
  ended_at timestamptz
);

CREATE TABLE audit_logs (
  id uuid PRIMARY KEY,
  branch_id uuid,
  user_id uuid,
  device_id uuid,
  action text NOT NULL,
  entity_type text NOT NULL,
  entity_id uuid,
  previous_data jsonb,
  new_data jsonb,
  reason text,
  created_at timestamptz NOT NULL DEFAULT now(),
  origin_event_id uuid
);
CREATE INDEX idx_audit_branch ON audit_logs(branch_id, created_at);
CREATE INDEX idx_audit_entity ON audit_logs(entity_type, entity_id);

CREATE TABLE app_settings (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  scope text NOT NULL,
  branch_id uuid,
  device_id uuid,
  key text NOT NULL,
  value jsonb NOT NULL,
  version integer NOT NULL DEFAULT 1,
  updated_by uuid,
  updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE sync_receipts (
  event_id uuid PRIMARY KEY,
  branch_id uuid NOT NULL,
  device_id uuid NOT NULL,
  local_sequence bigint NOT NULL,
  event_type text NOT NULL,
  payload_hash text NOT NULL,
  processed_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX idx_sync_receipts_device ON sync_receipts(device_id, local_sequence);
CREATE INDEX idx_sync_receipts_branch ON sync_receipts(branch_id, processed_at);
CREATE UNIQUE INDEX idx_sync_receipts_device_seq ON sync_receipts(device_id, local_sequence);

CREATE TABLE device_sequence_cloud (
  device_id uuid PRIMARY KEY REFERENCES devices(id),
  last_applied_sequence bigint NOT NULL DEFAULT 0
);

INSERT INTO payment_methods (id, code, name, name_ar, is_active, requires_reference, sort_order)
VALUES
  ('11111111-1111-1111-1111-111111111111', 'cash', 'Cash', 'نقدي', true, false, 1),
  ('22222222-2222-2222-2222-222222222222', 'card', 'Card', 'بطاقة', false, true, 2),
  ('33333333-3333-3333-3333-333333333333', 'other', 'Other', 'أخرى', false, true, 3);
