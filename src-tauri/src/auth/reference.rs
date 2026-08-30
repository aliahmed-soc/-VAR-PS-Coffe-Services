//! First-run hosted reference bootstrap. Rust is the only cloud client.

use std::collections::HashSet;

use chrono::Utc;
use reqwest::header::AUTHORIZATION;
use serde_json::{json, Value};
use sqlx::{SqlitePool, Transaction, Sqlite};

use crate::error::{AppError, AppResult};
use crate::sync::transport::{self, SupabaseConfig};

use super::pin;
use super::supabase_auth::{self, TokenResponse};

#[derive(Debug, Clone)]
pub struct ReferenceSnapshot {
    pub user_id: String,
    pub profiles: Vec<Value>,
    pub roles: Vec<Value>,
    pub branches: Vec<Value>,
    pub stations: Vec<Value>,
    pub pricing_rules: Vec<Value>,
    pub categories: Vec<Value>,
    pub products: Vec<Value>,
    pub branch_products: Vec<Value>,
    pub inventory_balances: Vec<Value>,
    pub payment_methods: Vec<Value>,
    pub devices: Vec<Value>,
}

#[derive(Debug, Clone)]
pub struct ResolvedAssignment {
    pub user_id: String,
    pub display_name: String,
    pub branch_id: String,
    pub role: String,
    pub is_system_admin: bool,
}

pub async fn complete_online_login(
    pool: &SqlitePool,
    cfg: &SupabaseConfig,
    email: &str,
    password: &str,
    pin: &str,
) -> AppResult<(ResolvedAssignment, TokenResponse)> {
    let tokens = supabase_auth::password_login(cfg, email, password).await?;
    let snapshot = match fetch_reference_snapshot(cfg, &tokens.access_token, &tokens.user.id).await {
        Ok(s) => s,
        Err(e) => return Err(e),
    };
    let assignment = apply_reference_and_resolve(pool, &snapshot, pin).await?;
    Ok((assignment, tokens))
}

pub async fn fetch_reference_snapshot(
    cfg: &SupabaseConfig,
    access_token: &str,
    user_id: &str,
) -> AppResult<ReferenceSnapshot> {
    let profiles = rest_select(
        cfg,
        access_token,
        &format!("user_profiles?user_id=eq.{user_id}&select=*"),
    )
    .await?;
    let roles = rest_select(cfg, access_token, "user_branch_roles?select=*").await?;
    let branches = rest_select(cfg, access_token, "branches?select=*").await?;
    let stations = rest_select(cfg, access_token, "stations?select=*").await?;
    let pricing_rules = rest_select(cfg, access_token, "pricing_rules?select=*").await?;
    let categories = rest_select(cfg, access_token, "categories?select=*").await?;
    let products = rest_select(cfg, access_token, "products?select=*").await?;
    let branch_products = rest_select(cfg, access_token, "branch_products?select=*").await?;
    let inventory_balances = rest_select(cfg, access_token, "inventory_balances?select=*").await?;
    let payment_methods = rest_select(cfg, access_token, "payment_methods?select=*").await?;
    let devices = rest_select(cfg, access_token, "devices?select=*").await?;
    Ok(ReferenceSnapshot {
        user_id: user_id.to_string(),
        profiles,
        roles,
        branches,
        stations,
        pricing_rules,
        categories,
        products,
        branch_products,
        inventory_balances,
        payment_methods,
        devices,
    })
}

pub async fn apply_reference_and_resolve(
    pool: &SqlitePool,
    snapshot: &ReferenceSnapshot,
    pin: &str,
) -> AppResult<ResolvedAssignment> {
    let assignment = validate_snapshot(snapshot)?;
    let mut tx = pool.begin().await?;
    persist_snapshot(&mut tx, snapshot).await?;
    tx.commit().await?;

    let hash = pin::hash_pin(pin)?;
    pin::cache_offline_access(
        pool,
        &assignment.user_id,
        &assignment.display_name,
        &assignment.branch_id,
        &assignment.role,
        &hash,
    )
    .await?;
    Ok(assignment)
}

pub fn validate_snapshot(snapshot: &ReferenceSnapshot) -> AppResult<ResolvedAssignment> {
    let profile = snapshot
        .profiles
        .iter()
        .find(|p| json_str(p, "user_id").as_deref() == Some(snapshot.user_id.as_str()))
        .ok_or_else(|| AppError::Auth("profile missing".into()))?;
    if json_bool_int(profile, "is_active", 1) != 1 {
        return Err(AppError::Auth("user is inactive".into()));
    }
    let is_admin = json_bool_int(profile, "is_system_admin", 0) == 1;
    let display = json_str(profile, "display_name").unwrap_or_else(|| "User".into());

    let active_roles: Vec<&Value> = snapshot
        .roles
        .iter()
        .filter(|r| json_str(r, "user_id").as_deref() == Some(snapshot.user_id.as_str()))
        .filter(|r| json_bool_int(r, "is_active", 0) == 1)
        .collect();
    if active_roles.is_empty() {
        return Err(AppError::Auth("no active branch assignment".into()));
    }
    if !is_admin && active_roles.len() != 1 {
        return Err(AppError::Auth(
            "cashier must have exactly one active branch".into(),
        ));
    }
    let role_row = active_roles[0];
    let branch_id = json_str(role_row, "branch_id")
        .ok_or_else(|| AppError::Auth("assignment missing branch".into()))?;
    let role = json_str(role_row, "role").unwrap_or_else(|| "cashier".into());
    if role != "admin" && role != "cashier" {
        return Err(AppError::Auth("invalid role".into()));
    }
    Ok(ResolvedAssignment {
        user_id: snapshot.user_id.clone(),
        display_name: display,
        branch_id,
        role,
        is_system_admin: is_admin,
    })
}

pub fn authorized_branch_ids(snapshot: &ReferenceSnapshot) -> HashSet<String> {
    snapshot
        .roles
        .iter()
        .filter(|r| json_str(r, "user_id").as_deref() == Some(snapshot.user_id.as_str()))
        .filter(|r| json_bool_int(r, "is_active", 0) == 1)
        .filter_map(|r| json_str(r, "branch_id"))
        .collect()
}

async fn persist_snapshot(
    tx: &mut Transaction<'_, Sqlite>,
    snapshot: &ReferenceSnapshot,
) -> AppResult<()> {
    let allowed = authorized_branch_ids(snapshot);
    if allowed.is_empty() {
        return Err(AppError::Auth("no authorized branch".into()));
    }

    for row in snapshot
        .branches
        .iter()
        .filter(|b| json_str(b, "id").is_some_and(|id| allowed.contains(&id)))
    {
        upsert_branch(tx, row).await?;
    }
    for row in &snapshot.profiles {
        upsert_profile(tx, row).await?;
    }
    for row in snapshot
        .roles
        .iter()
        .filter(|r| json_str(r, "user_id").as_deref() == Some(snapshot.user_id.as_str()))
        .filter(|r| json_str(r, "branch_id").is_some_and(|id| allowed.contains(&id)))
    {
        upsert_role(tx, row).await?;
    }
    for row in snapshot
        .devices
        .iter()
        .filter(|r| json_str(r, "branch_id").is_some_and(|id| allowed.contains(&id)))
    {
        upsert_device(tx, row).await?;
    }
    for row in snapshot
        .stations
        .iter()
        .filter(|r| json_str(r, "branch_id").is_some_and(|id| allowed.contains(&id)))
    {
        upsert_station(tx, row).await?;
    }
    for row in snapshot
        .pricing_rules
        .iter()
        .filter(|r| json_str(r, "branch_id").is_some_and(|id| allowed.contains(&id)))
    {
        upsert_pricing(tx, row).await?;
    }
    for row in &snapshot.categories {
        upsert_category(tx, row).await?;
    }
    for row in &snapshot.products {
        upsert_product(tx, row).await?;
    }
    for row in snapshot
        .branch_products
        .iter()
        .filter(|r| json_str(r, "branch_id").is_some_and(|id| allowed.contains(&id)))
    {
        upsert_branch_product(tx, row).await?;
    }
    for row in snapshot
        .inventory_balances
        .iter()
        .filter(|r| json_str(r, "branch_id").is_some_and(|id| allowed.contains(&id)))
    {
        upsert_inventory(tx, row).await?;
    }
    for row in &snapshot.payment_methods {
        upsert_payment_method(tx, row).await?;
    }

    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE sync_state SET last_reference_pull_at = ?, updated_at = ? WHERE id = 1",
    )
    .bind(&now)
    .bind(&now)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn rest_select(cfg: &SupabaseConfig, access_token: &str, query: &str) -> AppResult<Vec<Value>> {
    let client = transport::http_client();
    let url = format!("{}/rest/v1/{query}", cfg.url.trim_end_matches('/'));
    let res = client
        .get(url)
        .header(AUTHORIZATION, format!("Bearer {access_token}"))
        .header("apikey", &cfg.anon_key)
        .send()
        .await
        .map_err(|_| AppError::Auth("reference download failed".into()))?;
    let status = res.status();
    let body = res
        .text()
        .await
        .map_err(|_| AppError::Auth("reference download failed".into()))?;
    if !status.is_success() {
        let _ = body;
        return Err(AppError::Auth(format!(
            "reference download failed ({status})"
        )));
    }
    let value: Value =
        serde_json::from_str(&body).map_err(|_| AppError::Auth("reference download failed".into()))?;
    match value {
        Value::Array(rows) => Ok(rows),
        _ => Err(AppError::Auth("reference download failed".into())),
    }
}

fn json_str(row: &Value, key: &str) -> Option<String> {
    row.get(key).and_then(|v| {
        if v.is_null() {
            None
        } else {
            v.as_str().map(|s| s.to_string()).or_else(|| {
                if v.is_string() {
                    None
                } else {
                    Some(v.to_string().trim_matches('"').to_string())
                }
            })
        }
    })
}

fn json_i64(row: &Value, key: &str, default: i64) -> i64 {
    row.get(key)
        .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)))
        .unwrap_or(default)
}

fn json_bool_int(row: &Value, key: &str, default: i64) -> i64 {
    match row.get(key) {
        Some(Value::Bool(true)) => 1,
        Some(Value::Bool(false)) => 0,
        Some(Value::Number(n)) => n.as_i64().unwrap_or(default),
        Some(Value::String(s)) if s == "true" || s == "1" => 1,
        Some(Value::String(s)) if s == "false" || s == "0" => 0,
        _ => default,
    }
}

fn json_ts(row: &Value, key: &str) -> String {
    json_str(row, key).unwrap_or_else(|| Utc::now().to_rfc3339())
}

async fn upsert_branch(tx: &mut Transaction<'_, Sqlite>, row: &Value) -> AppResult<()> {
    let id = json_str(row, "id").ok_or_else(|| AppError::Auth("branch missing id".into()))?;
    sqlx::query(
        "INSERT INTO branches (id, code, name, timezone, currency_code, is_active, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET
           code=excluded.code, name=excluded.name, timezone=excluded.timezone,
           currency_code=excluded.currency_code, is_active=excluded.is_active, updated_at=excluded.updated_at",
    )
    .bind(&id)
    .bind(json_str(row, "code").unwrap_or_else(|| "UNK".into()))
    .bind(json_str(row, "name").unwrap_or_else(|| "Branch".into()))
    .bind(json_str(row, "timezone").unwrap_or_else(|| "Africa/Cairo".into()))
    .bind(json_str(row, "currency_code").unwrap_or_else(|| "EGP".into()))
    .bind(json_bool_int(row, "is_active", 1))
    .bind(json_ts(row, "created_at"))
    .bind(json_ts(row, "updated_at"))
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn upsert_profile(tx: &mut Transaction<'_, Sqlite>, row: &Value) -> AppResult<()> {
    let id = json_str(row, "user_id").ok_or_else(|| AppError::Auth("profile missing user_id".into()))?;
    sqlx::query(
        "INSERT INTO user_profiles (user_id, display_name, preferred_locale, is_system_admin, is_active, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(user_id) DO UPDATE SET
           display_name=excluded.display_name, preferred_locale=excluded.preferred_locale,
           is_system_admin=excluded.is_system_admin, is_active=excluded.is_active, updated_at=excluded.updated_at",
    )
    .bind(&id)
    .bind(json_str(row, "display_name").unwrap_or_else(|| "User".into()))
    .bind(json_str(row, "preferred_locale").unwrap_or_else(|| "en".into()))
    .bind(json_bool_int(row, "is_system_admin", 0))
    .bind(json_bool_int(row, "is_active", 1))
    .bind(json_ts(row, "created_at"))
    .bind(json_ts(row, "updated_at"))
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn upsert_role(tx: &mut Transaction<'_, Sqlite>, row: &Value) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO user_branch_roles (user_id, branch_id, role, offline_access_allowed, is_active, created_at)
         VALUES (?, ?, ?, ?, ?, ?)
         ON CONFLICT(user_id, branch_id) DO UPDATE SET
           role=excluded.role, offline_access_allowed=excluded.offline_access_allowed,
           is_active=excluded.is_active",
    )
    .bind(json_str(row, "user_id").ok_or_else(|| AppError::Auth("role missing user".into()))?)
    .bind(json_str(row, "branch_id").ok_or_else(|| AppError::Auth("role missing branch".into()))?)
    .bind(json_str(row, "role").unwrap_or_else(|| "cashier".into()))
    .bind(json_bool_int(row, "offline_access_allowed", 1))
    .bind(json_bool_int(row, "is_active", 1))
    .bind(json_ts(row, "created_at"))
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn upsert_device(tx: &mut Transaction<'_, Sqlite>, row: &Value) -> AppResult<()> {
    let id = json_str(row, "id").ok_or_else(|| AppError::Auth("device missing id".into()))?;
    sqlx::query(
        "INSERT INTO devices (id, branch_id, name, device_key, is_active, paired_at, last_seen_at, app_version)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET
           branch_id=excluded.branch_id, name=excluded.name, device_key=excluded.device_key,
           is_active=excluded.is_active, last_seen_at=excluded.last_seen_at, app_version=excluded.app_version",
    )
    .bind(&id)
    .bind(json_str(row, "branch_id").ok_or_else(|| AppError::Auth("device missing branch".into()))?)
    .bind(json_str(row, "name").unwrap_or_else(|| "Device".into()))
    .bind(json_str(row, "device_key").unwrap_or_else(|| id.clone()))
    .bind(json_bool_int(row, "is_active", 1))
    .bind(json_ts(row, "paired_at"))
    .bind(json_str(row, "last_seen_at"))
    .bind(json_str(row, "app_version"))
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn upsert_station(tx: &mut Transaction<'_, Sqlite>, row: &Value) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO stations (id, branch_id, code, display_name, sort_order, is_active)
         VALUES (?, ?, ?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET
           branch_id=excluded.branch_id, code=excluded.code, display_name=excluded.display_name,
           sort_order=excluded.sort_order, is_active=excluded.is_active",
    )
    .bind(json_str(row, "id").ok_or_else(|| AppError::Auth("station missing id".into()))?)
    .bind(json_str(row, "branch_id").ok_or_else(|| AppError::Auth("station missing branch".into()))?)
    .bind(json_str(row, "code").unwrap_or_else(|| "PS".into()))
    .bind(json_str(row, "display_name").unwrap_or_else(|| "Station".into()))
    .bind(json_i64(row, "sort_order", 0))
    .bind(json_bool_int(row, "is_active", 1))
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn upsert_pricing(tx: &mut Transaction<'_, Sqlite>, row: &Value) -> AppResult<()> {
    let rule_type = json_str(row, "rule_type").unwrap_or_default();
    if rule_type != "linear" {
        return Ok(());
    }
    sqlx::query(
        "INSERT INTO pricing_rules (
            id, branch_id, name, rule_type, rate_minor_per_hour,
            billing_increment_seconds, base_duration_seconds, base_charge_minor,
            step_duration_seconds, step_charge_minor, round_partial_step_up,
            version, effective_from, retired_at, created_by
         ) VALUES (?, ?, ?, 'linear', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET
           name=excluded.name, rate_minor_per_hour=excluded.rate_minor_per_hour,
           version=excluded.version, effective_from=excluded.effective_from, retired_at=excluded.retired_at",
    )
    .bind(json_str(row, "id").ok_or_else(|| AppError::Auth("pricing missing id".into()))?)
    .bind(json_str(row, "branch_id").ok_or_else(|| AppError::Auth("pricing missing branch".into()))?)
    .bind(json_str(row, "name").unwrap_or_else(|| "Linear".into()))
    .bind(json_i64(row, "rate_minor_per_hour", 0).max(0))
    .bind(row.get("billing_increment_seconds").and_then(|v| v.as_i64()))
    .bind(row.get("base_duration_seconds").and_then(|v| v.as_i64()))
    .bind(row.get("base_charge_minor").and_then(|v| v.as_i64()))
    .bind(row.get("step_duration_seconds").and_then(|v| v.as_i64()))
    .bind(row.get("step_charge_minor").and_then(|v| v.as_i64()))
    .bind(json_bool_int(row, "round_partial_step_up", 1))
    .bind(json_i64(row, "version", 1))
    .bind(json_ts(row, "effective_from"))
    .bind(json_str(row, "retired_at"))
    .bind(json_str(row, "created_by"))
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn upsert_category(tx: &mut Transaction<'_, Sqlite>, row: &Value) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO categories (id, name, name_ar, sort_order, is_active)
         VALUES (?, ?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET name=excluded.name, name_ar=excluded.name_ar,
           sort_order=excluded.sort_order, is_active=excluded.is_active",
    )
    .bind(json_str(row, "id").ok_or_else(|| AppError::Auth("category missing id".into()))?)
    .bind(json_str(row, "name").unwrap_or_else(|| "Category".into()))
    .bind(json_str(row, "name_ar"))
    .bind(json_i64(row, "sort_order", 0))
    .bind(json_bool_int(row, "is_active", 1))
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn upsert_product(tx: &mut Transaction<'_, Sqlite>, row: &Value) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO products (
            id, category_id, sku, barcode, name, name_ar,
            default_sell_price_minor, default_cost_price_minor, is_active, image_key, created_at, updated_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET
           category_id=excluded.category_id, sku=excluded.sku, name=excluded.name,
           default_sell_price_minor=excluded.default_sell_price_minor,
           default_cost_price_minor=excluded.default_cost_price_minor,
           is_active=excluded.is_active, updated_at=excluded.updated_at",
    )
    .bind(json_str(row, "id").ok_or_else(|| AppError::Auth("product missing id".into()))?)
    .bind(json_str(row, "category_id").ok_or_else(|| AppError::Auth("product missing category".into()))?)
    .bind(json_str(row, "sku"))
    .bind(json_str(row, "barcode"))
    .bind(json_str(row, "name").unwrap_or_else(|| "Product".into()))
    .bind(json_str(row, "name_ar"))
    .bind(json_i64(row, "default_sell_price_minor", 0).max(0))
    .bind(json_i64(row, "default_cost_price_minor", 0).max(0))
    .bind(json_bool_int(row, "is_active", 1))
    .bind(json_str(row, "image_key"))
    .bind(json_ts(row, "created_at"))
    .bind(json_ts(row, "updated_at"))
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn upsert_branch_product(tx: &mut Transaction<'_, Sqlite>, row: &Value) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO branch_products (branch_id, product_id, sell_price_override_minor, cost_price_override_minor, minimum_stock, is_active, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(branch_id, product_id) DO UPDATE SET
           sell_price_override_minor=excluded.sell_price_override_minor,
           cost_price_override_minor=excluded.cost_price_override_minor,
           minimum_stock=excluded.minimum_stock, is_active=excluded.is_active, updated_at=excluded.updated_at",
    )
    .bind(json_str(row, "branch_id").ok_or_else(|| AppError::Auth("branch_product missing branch".into()))?)
    .bind(json_str(row, "product_id").ok_or_else(|| AppError::Auth("branch_product missing product".into()))?)
    .bind(row.get("sell_price_override_minor").and_then(|v| v.as_i64()))
    .bind(row.get("cost_price_override_minor").and_then(|v| v.as_i64()))
    .bind(json_i64(row, "minimum_stock", 0))
    .bind(json_bool_int(row, "is_active", 1))
    .bind(json_ts(row, "updated_at"))
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn upsert_inventory(tx: &mut Transaction<'_, Sqlite>, row: &Value) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO inventory_balances (branch_id, product_id, quantity_on_hand, version, updated_at)
         VALUES (?, ?, ?, ?, ?)
         ON CONFLICT(branch_id, product_id) DO UPDATE SET
           quantity_on_hand=excluded.quantity_on_hand, version=excluded.version, updated_at=excluded.updated_at",
    )
    .bind(json_str(row, "branch_id").ok_or_else(|| AppError::Auth("inventory missing branch".into()))?)
    .bind(json_str(row, "product_id").ok_or_else(|| AppError::Auth("inventory missing product".into()))?)
    .bind(json_i64(row, "quantity_on_hand", 0).max(0))
    .bind(json_i64(row, "version", 0))
    .bind(json_ts(row, "updated_at"))
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn upsert_payment_method(tx: &mut Transaction<'_, Sqlite>, row: &Value) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO payment_methods (id, code, name, name_ar, is_active, requires_reference, sort_order)
         VALUES (?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET
           code=excluded.code, name=excluded.name, is_active=excluded.is_active, sort_order=excluded.sort_order",
    )
    .bind(json_str(row, "id").ok_or_else(|| AppError::Auth("payment method missing id".into()))?)
    .bind(json_str(row, "code").unwrap_or_else(|| "cash".into()))
    .bind(json_str(row, "name").unwrap_or_else(|| "Cash".into()))
    .bind(json_str(row, "name_ar"))
    .bind(json_bool_int(row, "is_active", 1))
    .bind(json_bool_int(row, "requires_reference", 0))
    .bind(json_i64(row, "sort_order", 0))
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub fn empty_snapshot(user_id: &str) -> ReferenceSnapshot {
    ReferenceSnapshot {
        user_id: user_id.into(),
        profiles: vec![],
        roles: vec![],
        branches: vec![],
        stations: vec![],
        pricing_rules: vec![],
        categories: vec![],
        products: vec![],
        branch_products: vec![],
        inventory_balances: vec![],
        payment_methods: vec![],
        devices: vec![],
    }
}

pub fn fixture_cashier_b1() -> ReferenceSnapshot {
    let uid = "a11e0001-0a11-4000-a000-000000000002";
    let b1 = "a11e0001-0a11-4000-b000-000000000001";
    let b2 = "a11e0001-0a11-4000-b000-000000000002";
    let cat = "a11e0001-0a11-4000-c000-000000000001";
    let drink = "a11e0001-0a11-4000-c000-000000000011";
    let snack = "a11e0001-0a11-4000-c000-000000000012";
    let now = "2026-08-30T00:00:00Z";
    // Hosted RLS would omit B2. Include a B2 row here only to prove the writer filters it out.
    ReferenceSnapshot {
        user_id: uid.into(),
        profiles: vec![json!({
            "user_id": uid, "display_name": "UAT B1 Cashier", "preferred_locale": "ar",
            "is_system_admin": false, "is_active": true, "created_at": now, "updated_at": now
        })],
        roles: vec![json!({
            "user_id": uid, "branch_id": b1, "role": "cashier",
            "offline_access_allowed": true, "is_active": true, "created_at": now
        })],
        branches: vec![
            json!({"id": b1, "code": "UAT1", "name": "UAT Branch 1", "timezone": "Africa/Cairo", "currency_code": "EGP", "is_active": true, "created_at": now, "updated_at": now}),
            json!({"id": b2, "code": "UAT2", "name": "UAT Branch 2", "timezone": "Africa/Cairo", "currency_code": "EGP", "is_active": true, "created_at": now, "updated_at": now}),
        ],
        stations: vec![
            json!({"id": "a11e0001-0a11-4000-5001-000000000001", "branch_id": b1, "code": "PS1", "display_name": "UAT B1 PS1", "sort_order": 1, "is_active": true}),
            json!({"id": "a11e0001-0a11-4000-5002-000000000001", "branch_id": b2, "code": "PS1", "display_name": "UAT B2 PS1", "sort_order": 1, "is_active": true}),
        ],
        pricing_rules: vec![
            json!({"id": "a11e0001-0a11-4000-e000-000000000001", "branch_id": b1, "name": "UAT Linear Test Rate", "rule_type": "linear", "rate_minor_per_hour": 3000, "effective_from": now}),
            json!({"id": "a11e0001-0a11-4000-e000-000000000002", "branch_id": b2, "name": "UAT Linear Test Rate", "rule_type": "linear", "rate_minor_per_hour": 3000, "effective_from": now}),
        ],
        categories: vec![json!({"id": cat, "name": "UAT Category", "name_ar": "فئة اختبار", "sort_order": 1, "is_active": true})],
        products: vec![
            json!({"id": drink, "category_id": cat, "sku": "UAT-DRINK", "name": "UAT Drink", "default_sell_price_minor": 1500, "default_cost_price_minor": 700, "is_active": true, "created_at": now, "updated_at": now}),
            json!({"id": snack, "category_id": cat, "sku": "UAT-SNACK", "name": "UAT Snack", "default_sell_price_minor": 1000, "default_cost_price_minor": 400, "is_active": true, "created_at": now, "updated_at": now}),
        ],
        branch_products: vec![
            json!({"branch_id": b1, "product_id": drink, "minimum_stock": 2, "is_active": true, "updated_at": now}),
            json!({"branch_id": b2, "product_id": drink, "minimum_stock": 2, "is_active": true, "updated_at": now}),
        ],
        inventory_balances: vec![
            json!({"branch_id": b1, "product_id": drink, "quantity_on_hand": 20, "version": 0, "updated_at": now}),
            json!({"branch_id": b2, "product_id": drink, "quantity_on_hand": 20, "version": 0, "updated_at": now}),
        ],
        payment_methods: vec![json!({
            "id": "11111111-1111-1111-1111-111111111111", "code": "cash", "name": "Cash",
            "name_ar": "نقدي", "is_active": true, "requires_reference": false, "sort_order": 1
        })],
        devices: vec![
            json!({"id": "a11e0001-0a11-4000-d000-000000000001", "branch_id": b1, "name": "UAT B1 Writer", "device_key": "uat-dev-b1", "is_active": true, "paired_at": now}),
            json!({"id": "a11e0001-0a11-4000-d000-000000000002", "branch_id": b2, "name": "UAT B2 Writer", "device_key": "uat-dev-b2", "is_active": true, "paired_at": now}),
        ],
    }
}

pub fn fixture_cashier_b2() -> ReferenceSnapshot {
    let mut snap = fixture_cashier_b1();
    snap.user_id = "a11e0001-0a11-4000-a000-000000000003".into();
    snap.profiles = vec![json!({
        "user_id": snap.user_id, "display_name": "UAT B2 Cashier", "preferred_locale": "ar",
        "is_system_admin": false, "is_active": true, "created_at": "2026-08-30T00:00:00Z", "updated_at": "2026-08-30T00:00:00Z"
    })];
    snap.roles = vec![json!({
        "user_id": snap.user_id, "branch_id": "a11e0001-0a11-4000-b000-000000000002",
        "role": "cashier", "offline_access_allowed": true, "is_active": true, "created_at": "2026-08-30T00:00:00Z"
    })];
    snap
}
