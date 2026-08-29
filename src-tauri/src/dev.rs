use chrono::Utc;
use sqlx::SqlitePool;

use crate::auth::pin;
use crate::error::AppResult;

pub async fn seed_two_branches(pool: &SqlitePool) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT OR IGNORE INTO branches (id, code, name, timezone, currency_code, is_active, created_at, updated_at)
         VALUES
         ('b1','B1','Branch 1','Africa/Cairo','EGP',1,?,?),
         ('b2','B2','Branch 2','Africa/Cairo','EGP',1,?,?)",
    )
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;

    sqlx::query(
        "INSERT OR IGNORE INTO user_profiles (user_id, display_name, preferred_locale, is_system_admin, is_active, created_at, updated_at)
         VALUES ('u-admin','Admin','en',1,1,?,?), ('u-c1','Ahmed','ar',0,1,?,?)",
    )
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;

    sqlx::query(
        "INSERT OR IGNORE INTO user_branch_roles (user_id, branch_id, role, offline_access_allowed, is_active, created_at)
         VALUES ('u-admin','b1','admin',1,1,?), ('u-c1','b1','cashier',1,1,?), ('u-admin','b2','admin',1,1,?)",
    )
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;

    sqlx::query(
        "INSERT OR IGNORE INTO stations (id, branch_id, code, display_name, sort_order, is_active)
         VALUES ('s-ps1','b1','PS1','PS1',1,1), ('s-ps2','b1','PS2','PS2',2,1), ('s-b2-ps1','b2','PS1','PS1',1,1)",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "INSERT OR IGNORE INTO pricing_rules (
            id, branch_id, name, rule_type, rate_minor_per_hour, billing_increment_seconds,
            version, effective_from, round_partial_step_up
         ) VALUES ('pr-b1','b1','Linear 30','linear',3000,NULL,1,?,1),
                  ('pr-b2','b2','Linear 30','linear',3000,NULL,1,?,1)",
    )
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;

    sqlx::query(
        "INSERT OR IGNORE INTO categories (id, name, name_ar, sort_order, is_active)
         VALUES ('cat-drinks','Drinks','مشروبات',1,1)",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "INSERT OR IGNORE INTO products (id, category_id, sku, name, name_ar, default_sell_price_minor, default_cost_price_minor, is_active, created_at, updated_at)
         VALUES
         ('p-coke','cat-drinks','COKE','Coca-Cola','كوكا كولا',2500,1000,1,?,?),
         ('p-water','cat-drinks','WATER','Water','مياه',1000,400,1,?,?),
         ('p-chips','cat-drinks','CHIPS','Chips','شيبسي',1500,600,1,?,?)",
    )
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;

    sqlx::query(
        "INSERT OR IGNORE INTO branch_products (branch_id, product_id, minimum_stock, is_active, updated_at)
         VALUES
         ('b1','p-coke',5,1,?), ('b2','p-coke',5,1,?),
         ('b1','p-water',5,1,?), ('b2','p-water',5,1,?),
         ('b1','p-chips',5,1,?), ('b2','p-chips',5,1,?)",
    )
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;

    sqlx::query(
        "INSERT OR IGNORE INTO inventory_balances (branch_id, product_id, quantity_on_hand, version, updated_at)
         VALUES
         ('b1','p-coke',50,1,?), ('b2','p-coke',80,1,?),
         ('b1','p-water',80,1,?), ('b2','p-water',80,1,?),
         ('b1','p-chips',40,1,?), ('b2','p-chips',40,1,?)",
    )
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;

    sqlx::query(
        "INSERT OR IGNORE INTO devices (id, branch_id, name, device_key, is_active, paired_at)
         VALUES ('d1','b1','Cashier 1','dev-key-b1',1,?)",
    )
    .bind(&now)
    .execute(pool)
    .await?;

    let pin_hash = pin::hash_pin("1357")?;
    pin::cache_offline_access(pool, "u-c1", "Ahmed", "b1", "cashier", &pin_hash).await?;
    pin::cache_offline_access(pool, "u-admin", "Admin", "b1", "admin", &pin_hash).await?;
    Ok(())
}
