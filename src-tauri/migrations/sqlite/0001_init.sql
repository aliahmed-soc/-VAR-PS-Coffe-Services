-- PlayStation Café POS — SQLite operational schema
PRAGMA foreign_keys = ON;

CREATE TABLE branches (
  id TEXT PRIMARY KEY NOT NULL,
  code TEXT NOT NULL UNIQUE,
  name TEXT NOT NULL,
  timezone TEXT NOT NULL DEFAULT 'Africa/Cairo',
  currency_code TEXT NOT NULL DEFAULT 'EGP' CHECK (length(currency_code) = 3),
  is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE user_profiles (
  user_id TEXT PRIMARY KEY NOT NULL,
  display_name TEXT NOT NULL,
  preferred_locale TEXT NOT NULL DEFAULT 'en' CHECK (preferred_locale IN ('en', 'ar')),
  is_system_admin INTEGER NOT NULL DEFAULT 0 CHECK (is_system_admin IN (0, 1)),
  is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE user_branch_roles (
  user_id TEXT NOT NULL,
  branch_id TEXT NOT NULL,
  role TEXT NOT NULL CHECK (role IN ('admin', 'cashier')),
  offline_access_allowed INTEGER NOT NULL DEFAULT 1 CHECK (offline_access_allowed IN (0, 1)),
  is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
  created_at TEXT NOT NULL,
  PRIMARY KEY (user_id, branch_id),
  FOREIGN KEY (user_id) REFERENCES user_profiles(user_id),
  FOREIGN KEY (branch_id) REFERENCES branches(id)
);
CREATE INDEX idx_user_branch_roles_branch ON user_branch_roles(branch_id);

CREATE TABLE devices (
  id TEXT PRIMARY KEY NOT NULL,
  branch_id TEXT NOT NULL,
  name TEXT NOT NULL,
  device_key TEXT NOT NULL UNIQUE,
  is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
  paired_at TEXT NOT NULL,
  last_seen_at TEXT,
  app_version TEXT,
  FOREIGN KEY (branch_id) REFERENCES branches(id)
);

CREATE TABLE stations (
  id TEXT PRIMARY KEY NOT NULL,
  branch_id TEXT NOT NULL,
  code TEXT NOT NULL,
  display_name TEXT NOT NULL,
  sort_order INTEGER NOT NULL DEFAULT 0,
  is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
  UNIQUE (branch_id, code),
  FOREIGN KEY (branch_id) REFERENCES branches(id)
);

CREATE TABLE pricing_rules (
  id TEXT PRIMARY KEY NOT NULL,
  branch_id TEXT NOT NULL,
  name TEXT NOT NULL,
  rule_type TEXT NOT NULL CHECK (rule_type IN ('linear', 'stepped')),
  rate_minor_per_hour INTEGER,
  billing_increment_seconds INTEGER,
  base_duration_seconds INTEGER,
  base_charge_minor INTEGER,
  step_duration_seconds INTEGER,
  step_charge_minor INTEGER,
  round_partial_step_up INTEGER NOT NULL DEFAULT 1 CHECK (round_partial_step_up IN (0, 1)),
  version INTEGER NOT NULL DEFAULT 1,
  effective_from TEXT NOT NULL,
  retired_at TEXT,
  created_by TEXT,
  FOREIGN KEY (branch_id) REFERENCES branches(id)
);

CREATE TABLE categories (
  id TEXT PRIMARY KEY NOT NULL,
  name TEXT NOT NULL,
  name_ar TEXT,
  sort_order INTEGER NOT NULL DEFAULT 0,
  is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1))
);

CREATE TABLE products (
  id TEXT PRIMARY KEY NOT NULL,
  category_id TEXT NOT NULL,
  sku TEXT,
  barcode TEXT,
  name TEXT NOT NULL,
  name_ar TEXT,
  default_sell_price_minor INTEGER NOT NULL CHECK (default_sell_price_minor >= 0),
  default_cost_price_minor INTEGER NOT NULL CHECK (default_cost_price_minor >= 0),
  is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
  image_key TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY (category_id) REFERENCES categories(id)
);
CREATE UNIQUE INDEX idx_products_sku ON products(sku) WHERE sku IS NOT NULL;
CREATE UNIQUE INDEX idx_products_barcode ON products(barcode) WHERE barcode IS NOT NULL;
CREATE INDEX idx_products_name ON products(name);
CREATE INDEX idx_products_category ON products(category_id);

CREATE TABLE branch_products (
  branch_id TEXT NOT NULL,
  product_id TEXT NOT NULL,
  sell_price_override_minor INTEGER,
  cost_price_override_minor INTEGER,
  minimum_stock INTEGER NOT NULL DEFAULT 0,
  is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
  updated_at TEXT NOT NULL,
  PRIMARY KEY (branch_id, product_id),
  FOREIGN KEY (branch_id) REFERENCES branches(id),
  FOREIGN KEY (product_id) REFERENCES products(id)
);

CREATE TABLE inventory_balances (
  branch_id TEXT NOT NULL,
  product_id TEXT NOT NULL,
  quantity_on_hand INTEGER NOT NULL CHECK (quantity_on_hand >= 0),
  version INTEGER NOT NULL DEFAULT 0,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (branch_id, product_id),
  FOREIGN KEY (branch_id) REFERENCES branches(id),
  FOREIGN KEY (product_id) REFERENCES products(id)
);

CREATE TABLE orders (
  id TEXT PRIMARY KEY NOT NULL,
  branch_id TEXT NOT NULL,
  order_type TEXT NOT NULL CHECK (order_type IN ('gaming', 'pos')),
  status TEXT NOT NULL CHECK (status IN ('open', 'checkout_pending', 'paid', 'void', 'refunded')),
  product_subtotal_minor INTEGER NOT NULL DEFAULT 0 CHECK (product_subtotal_minor >= 0),
  gaming_subtotal_minor INTEGER NOT NULL DEFAULT 0 CHECK (gaming_subtotal_minor >= 0),
  subtotal_minor INTEGER NOT NULL DEFAULT 0 CHECK (subtotal_minor >= 0),
  discount_minor INTEGER NOT NULL DEFAULT 0 CHECK (discount_minor >= 0),
  tax_minor INTEGER NOT NULL DEFAULT 0 CHECK (tax_minor >= 0),
  tax_rate_bps INTEGER NOT NULL DEFAULT 0 CHECK (tax_rate_bps >= 0),
  total_minor INTEGER NOT NULL DEFAULT 0 CHECK (total_minor >= 0),
  amount_paid_minor INTEGER NOT NULL DEFAULT 0 CHECK (amount_paid_minor >= 0),
  change_minor INTEGER NOT NULL DEFAULT 0 CHECK (change_minor >= 0),
  currency_code TEXT NOT NULL DEFAULT 'EGP' CHECK (length(currency_code) = 3),
  receipt_number TEXT,
  receipt_snapshot TEXT,
  origin_device_id TEXT,
  opened_by TEXT NOT NULL,
  closed_by TEXT,
  opened_at TEXT NOT NULL,
  closed_at TEXT,
  FOREIGN KEY (branch_id) REFERENCES branches(id),
  CHECK (subtotal_minor = product_subtotal_minor + gaming_subtotal_minor),
  CHECK (total_minor = subtotal_minor + tax_minor - discount_minor)
);
CREATE INDEX idx_orders_branch_status ON orders(branch_id, status);
CREATE INDEX idx_orders_branch_opened ON orders(branch_id, opened_at);
CREATE UNIQUE INDEX idx_orders_receipt ON orders(receipt_number) WHERE receipt_number IS NOT NULL;

CREATE TRIGGER orders_paid_tax_immutable
BEFORE UPDATE ON orders
FOR EACH ROW
WHEN OLD.status = 'paid'
 AND (
   NEW.tax_minor IS NOT OLD.tax_minor
   OR NEW.tax_rate_bps IS NOT OLD.tax_rate_bps
   OR NEW.subtotal_minor IS NOT OLD.subtotal_minor
 )
BEGIN
  SELECT RAISE(ABORT, 'paid_tax_immutable');
END;

CREATE TABLE order_items (
  id TEXT PRIMARY KEY NOT NULL,
  branch_id TEXT NOT NULL,
  order_id TEXT NOT NULL,
  product_id TEXT NOT NULL,
  product_name_snapshot TEXT NOT NULL,
  quantity INTEGER NOT NULL CHECK (quantity > 0),
  unit_price_minor INTEGER NOT NULL CHECK (unit_price_minor >= 0),
  unit_cost_minor INTEGER NOT NULL CHECK (unit_cost_minor >= 0),
  line_total_minor INTEGER NOT NULL CHECK (line_total_minor >= 0),
  status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'voided')),
  added_by TEXT NOT NULL,
  added_at TEXT NOT NULL,
  voided_at TEXT,
  void_reason TEXT,
  FOREIGN KEY (branch_id) REFERENCES branches(id),
  FOREIGN KEY (order_id) REFERENCES orders(id),
  FOREIGN KEY (product_id) REFERENCES products(id)
);
CREATE INDEX idx_order_items_order ON order_items(order_id);

CREATE TABLE gaming_sessions (
  id TEXT PRIMARY KEY NOT NULL,
  branch_id TEXT NOT NULL,
  station_id TEXT NOT NULL,
  order_id TEXT NOT NULL UNIQUE,
  status TEXT NOT NULL CHECK (status IN ('active', 'stopped', 'void')),
  started_at TEXT NOT NULL,
  ended_at TEXT,
  duration_seconds INTEGER,
  pricing_rule_id TEXT,
  pricing_snapshot TEXT NOT NULL,
  calculated_charge_minor INTEGER,
  final_charge_minor INTEGER,
  started_by TEXT NOT NULL,
  stopped_by TEXT,
  clock_anomaly INTEGER NOT NULL DEFAULT 0 CHECK (clock_anomaly IN (0, 1)),
  FOREIGN KEY (branch_id) REFERENCES branches(id),
  FOREIGN KEY (station_id) REFERENCES stations(id),
  FOREIGN KEY (order_id) REFERENCES orders(id)
);
CREATE UNIQUE INDEX idx_gaming_one_active_station
  ON gaming_sessions(branch_id, station_id) WHERE status = 'active';

CREATE TABLE inventory_movements (
  id TEXT PRIMARY KEY NOT NULL,
  branch_id TEXT NOT NULL,
  product_id TEXT NOT NULL,
  movement_type TEXT NOT NULL CHECK (movement_type IN (
    'opening', 'sale', 'sale_void', 'adjustment_in', 'adjustment_out',
    'damaged', 'expired', 'refund', 'transfer_in', 'transfer_out'
  )),
  quantity_delta INTEGER NOT NULL,
  quantity_after INTEGER NOT NULL CHECK (quantity_after >= 0),
  order_id TEXT,
  order_item_id TEXT,
  reason TEXT,
  origin_event_id TEXT NOT NULL UNIQUE,
  created_by TEXT NOT NULL,
  created_at TEXT NOT NULL,
  FOREIGN KEY (branch_id) REFERENCES branches(id),
  FOREIGN KEY (product_id) REFERENCES products(id)
);
CREATE INDEX idx_inv_movements_product ON inventory_movements(branch_id, product_id, created_at);
CREATE INDEX idx_inv_movements_order ON inventory_movements(order_id);

CREATE TABLE payment_methods (
  id TEXT PRIMARY KEY NOT NULL,
  code TEXT NOT NULL UNIQUE,
  name TEXT NOT NULL,
  name_ar TEXT,
  is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
  requires_reference INTEGER NOT NULL DEFAULT 0 CHECK (requires_reference IN (0, 1)),
  sort_order INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE payments (
  id TEXT PRIMARY KEY NOT NULL,
  branch_id TEXT NOT NULL,
  order_id TEXT NOT NULL,
  payment_method_id TEXT NOT NULL,
  payment_type TEXT NOT NULL CHECK (payment_type IN ('sale', 'refund', 'reversal')),
  amount_due_minor INTEGER NOT NULL CHECK (amount_due_minor >= 0),
  amount_tendered_minor INTEGER NOT NULL CHECK (amount_tendered_minor >= 0),
  amount_applied_minor INTEGER NOT NULL CHECK (amount_applied_minor >= 0),
  change_minor INTEGER NOT NULL CHECK (change_minor >= 0),
  status TEXT NOT NULL CHECK (status IN ('captured', 'voided', 'refunded', 'reversed')),
  parent_payment_id TEXT,
  reference TEXT,
  cashier_id TEXT NOT NULL,
  paid_at TEXT NOT NULL,
  origin_event_id TEXT NOT NULL UNIQUE,
  FOREIGN KEY (branch_id) REFERENCES branches(id),
  FOREIGN KEY (order_id) REFERENCES orders(id),
  FOREIGN KEY (payment_method_id) REFERENCES payment_methods(id)
);
CREATE UNIQUE INDEX idx_payments_one_captured_sale
  ON payments(order_id) WHERE payment_type = 'sale' AND status = 'captured';

CREATE TABLE audit_logs (
  id TEXT PRIMARY KEY NOT NULL,
  branch_id TEXT,
  user_id TEXT,
  device_id TEXT,
  action TEXT NOT NULL,
  entity_type TEXT NOT NULL,
  entity_id TEXT,
  previous_data TEXT,
  new_data TEXT,
  reason TEXT,
  created_at TEXT NOT NULL,
  origin_event_id TEXT
);
CREATE INDEX idx_audit_branch ON audit_logs(branch_id, created_at);
CREATE INDEX idx_audit_entity ON audit_logs(entity_type, entity_id);

CREATE TABLE app_settings (
  id TEXT PRIMARY KEY NOT NULL,
  scope TEXT NOT NULL,
  branch_id TEXT,
  device_id TEXT,
  key TEXT NOT NULL,
  value TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1,
  updated_by TEXT,
  updated_at TEXT NOT NULL
);

CREATE TABLE sync_outbox (
  event_id TEXT PRIMARY KEY NOT NULL,
  sequence INTEGER NOT NULL,
  event_type TEXT NOT NULL,
  aggregate_type TEXT NOT NULL,
  aggregate_id TEXT NOT NULL,
  branch_id TEXT NOT NULL,
  device_id TEXT NOT NULL,
  payload_json TEXT NOT NULL,
  payload_hash TEXT NOT NULL,
  created_at TEXT NOT NULL,
  attempt_count INTEGER NOT NULL DEFAULT 0,
  next_attempt_at TEXT NOT NULL,
  last_error TEXT,
  sync_status TEXT NOT NULL DEFAULT 'pending' CHECK (sync_status IN ('pending', 'sending', 'synced', 'failed'))
);
CREATE INDEX idx_outbox_status_next ON sync_outbox(sync_status, next_attempt_at);
CREATE UNIQUE INDEX idx_outbox_device_seq ON sync_outbox(device_id, sequence);

CREATE TABLE sync_state (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  last_successful_push_at TEXT,
  last_successful_pull_at TEXT,
  last_reference_pull_at TEXT,
  cloud_connectivity TEXT NOT NULL DEFAULT 'unknown',
  reference_data_version TEXT,
  last_applied_cloud_sequence INTEGER,
  restore_reconciliation_required INTEGER NOT NULL DEFAULT 0 CHECK (restore_reconciliation_required IN (0, 1)),
  pending_count INTEGER NOT NULL DEFAULT 0,
  updated_at TEXT NOT NULL
);

CREATE TABLE offline_access_cache (
  user_id TEXT PRIMARY KEY NOT NULL,
  display_name TEXT NOT NULL,
  branch_id TEXT NOT NULL,
  role TEXT NOT NULL CHECK (role IN ('admin', 'cashier')),
  pin_hash TEXT NOT NULL,
  authorization_expires_at TEXT NOT NULL,
  last_online_auth_at TEXT NOT NULL
);

CREATE TABLE device_sequence (
  device_id TEXT PRIMARY KEY NOT NULL,
  next_sequence INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE IF NOT EXISTS schema_migrations (
  version INTEGER PRIMARY KEY NOT NULL,
  name TEXT NOT NULL,
  applied_at TEXT NOT NULL
);
