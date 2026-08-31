use chrono::Utc;
use sqlx::{SqliteConnection, SqlitePool};
use uuid::Uuid;

use super::gaming::insert_audit;
use crate::error::{AppError, AppResult};
use crate::sync::outbox;

pub(crate) async fn apply_stock_cas(
    tx: &mut SqliteConnection,
    branch_id: &str,
    product_id: &str,
    expected_qty: i64,
    expected_version: i64,
    after: i64,
    now: &str,
) -> AppResult<()> {
    let updated = sqlx::query(
        "UPDATE inventory_balances
         SET quantity_on_hand = ?, version = version + 1, updated_at = ?
         WHERE branch_id = ? AND product_id = ?
           AND quantity_on_hand = ? AND version = ?",
    )
    .bind(after)
    .bind(now)
    .bind(branch_id)
    .bind(product_id)
    .bind(expected_qty)
    .bind(expected_version)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if updated != 1 {
        return Err(AppError::Conflict("inventory version conflict".into()));
    }
    Ok(())
}

pub async fn add_product_to_order(
    pool: &SqlitePool,
    branch_id: &str,
    device_id: &str,
    order_id: &str,
    product_id: &str,
    quantity: i64,
    user_id: &str,
) -> AppResult<serde_json::Value> {
    if quantity <= 0 {
        return Err(AppError::domain("quantity must be > 0"));
    }
    let mut tx = pool.begin().await?;
    let order: (String, String) =
        sqlx::query_as("SELECT status, branch_id FROM orders WHERE id = ?")
            .bind(order_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| AppError::NotFound("order".into()))?;
    if order.1 != branch_id {
        return Err(AppError::Forbidden("cross-branch order".into()));
    }
    if order.0 == "paid" {
        return Err(AppError::Conflict("paid order cannot be modified".into()));
    }
    if order.0 == "void" {
        return Err(AppError::Conflict("void order cannot be modified".into()));
    }

    let product: (String, i64, i64) = sqlx::query_as(
        "SELECT p.name,
                COALESCE(bp.sell_price_override_minor, p.default_sell_price_minor),
                COALESCE(bp.cost_price_override_minor, p.default_cost_price_minor)
         FROM products p
         JOIN branch_products bp ON bp.product_id = p.id AND bp.branch_id = ?
         WHERE p.id = ? AND p.is_active = 1 AND bp.is_active = 1",
    )
    .bind(branch_id)
    .bind(product_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound("product not available at branch".into()))?;

    let stock_row: Option<(i64, i64)> = sqlx::query_as(
        "SELECT quantity_on_hand, version FROM inventory_balances WHERE branch_id = ? AND product_id = ?",
    )
    .bind(branch_id)
    .bind(product_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((stock, version)) = stock_row else {
        return Err(AppError::Conflict("insufficient stock".into()));
    };
    if stock < quantity {
        return Err(AppError::Conflict("insufficient stock".into()));
    }

    let unit_price = product.1;
    let unit_cost = product.2;
    let line_total = unit_price.saturating_mul(quantity);
    let after = stock - quantity;
    let item_id = Uuid::new_v4().to_string();
    let movement_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let event_id_placeholder = Uuid::new_v4().to_string();

    sqlx::query(
        "INSERT INTO order_items (
            id, branch_id, order_id, product_id, product_name_snapshot, quantity,
            unit_price_minor, unit_cost_minor, line_total_minor, status, added_by, added_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'active', ?, ?)",
    )
    .bind(&item_id)
    .bind(branch_id)
    .bind(order_id)
    .bind(product_id)
    .bind(&product.0)
    .bind(quantity)
    .bind(unit_price)
    .bind(unit_cost)
    .bind(line_total)
    .bind(user_id)
    .bind(&now)
    .execute(&mut *tx)
    .await?;

    apply_stock_cas(&mut tx, branch_id, product_id, stock, version, after, &now).await?;

    sqlx::query(
        "UPDATE orders
         SET product_subtotal_minor = product_subtotal_minor + ?,
             subtotal_minor = product_subtotal_minor + ? + gaming_subtotal_minor,
             total_minor = product_subtotal_minor + ? + gaming_subtotal_minor - discount_minor + tax_minor
         WHERE id = ?",
    )
    .bind(line_total)
    .bind(line_total)
    .bind(line_total)
    .bind(order_id)
    .execute(&mut *tx)
    .await?;

    let payload = serde_json::json!({
        "order_item_id": item_id,
        "order_id": order_id,
        "branch_id": branch_id,
        "product_id": product_id,
        "product_name_snapshot": product.0,
        "quantity": quantity,
        "unit_price_minor": unit_price,
        "unit_cost_minor": unit_cost,
        "line_total_minor": line_total,
        "added_by": user_id,
        "added_at": now,
        "movement_id": movement_id
    });
    let out = outbox::enqueue(
        &mut tx,
        device_id,
        branch_id,
        "order.item_added",
        &item_id,
        payload.clone(),
    )
    .await?;

    sqlx::query(
        "INSERT INTO inventory_movements (
            id, branch_id, product_id, movement_type, quantity_delta, quantity_after,
            order_id, order_item_id, origin_event_id, created_by, created_at
         ) VALUES (?, ?, ?, 'sale', ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&movement_id)
    .bind(branch_id)
    .bind(product_id)
    .bind(-quantity)
    .bind(after)
    .bind(order_id)
    .bind(&item_id)
    .bind(&out.event_id)
    .bind(user_id)
    .bind(&now)
    .execute(&mut *tx)
    .await?;
    let _ = event_id_placeholder;

    insert_audit(
        &mut tx,
        branch_id,
        user_id,
        device_id,
        "order.item_added",
        "order_item",
        &item_id,
        None,
        Some(&payload),
    )
    .await?;
    tx.commit().await?;
    Ok(payload)
}

pub async fn void_order_item(
    pool: &SqlitePool,
    branch_id: &str,
    device_id: &str,
    item_id: &str,
    user_id: &str,
    reason: &str,
) -> AppResult<serde_json::Value> {
    let mut tx = pool.begin().await?;
    let payload = void_item_in_tx(&mut tx, branch_id, device_id, item_id, user_id, reason).await?;
    tx.commit().await?;
    Ok(payload)
}

/// Voids one line inside a caller's transaction, so voiding a whole ticket can
/// retire all of its lines atomically instead of one commit at a time.
pub(crate) async fn void_item_in_tx(
    tx: &mut SqliteConnection,
    branch_id: &str,
    device_id: &str,
    item_id: &str,
    user_id: &str,
    reason: &str,
) -> AppResult<serde_json::Value> {
    let item: Option<(String, String, String, i64, i64, String)> = sqlx::query_as(
        "SELECT order_id, product_id, status, quantity, line_total_minor, branch_id FROM order_items WHERE id = ?",
    )
    .bind(item_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((order_id, product_id, status, quantity, line_total, item_branch)) = item else {
        return Err(AppError::NotFound("order item".into()));
    };
    if item_branch != branch_id {
        return Err(AppError::Forbidden("cross-branch item".into()));
    }
    if status != "active" {
        return Err(AppError::Conflict("item already voided".into()));
    }
    let order_status: String = sqlx::query_scalar("SELECT status FROM orders WHERE id = ?")
        .bind(&order_id)
        .fetch_one(&mut *tx)
        .await?;
    if order_status == "paid" {
        return Err(AppError::Conflict("paid order cannot be modified".into()));
    }

    let (stock, version): (i64, i64) = sqlx::query_as(
        "SELECT quantity_on_hand, version FROM inventory_balances WHERE branch_id = ? AND product_id = ?",
    )
    .bind(branch_id)
    .bind(&product_id)
    .fetch_one(&mut *tx)
    .await?;
    let after = stock + quantity;
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        "UPDATE order_items SET status = 'voided', voided_at = ?, void_reason = ? WHERE id = ?",
    )
    .bind(&now)
    .bind(reason)
    .bind(item_id)
    .execute(&mut *tx)
    .await?;
    apply_stock_cas(&mut *tx, branch_id, &product_id, stock, version, after, &now).await?;
    sqlx::query(
        "UPDATE orders SET product_subtotal_minor = product_subtotal_minor - ?,
            subtotal_minor = product_subtotal_minor - ? + gaming_subtotal_minor,
            total_minor = product_subtotal_minor - ? + gaming_subtotal_minor - discount_minor + tax_minor
         WHERE id = ?",
    )
    .bind(line_total)
    .bind(line_total)
    .bind(line_total)
    .bind(&order_id)
    .execute(&mut *tx)
    .await?;

    let payload = serde_json::json!({
        "order_item_id": item_id,
        "order_id": order_id,
        "branch_id": branch_id,
        "quantity": quantity,
        "voided_by": user_id,
        "void_reason": reason
    });
    let out = outbox::enqueue(
        &mut *tx,
        device_id,
        branch_id,
        "order.item_voided",
        item_id,
        payload.clone(),
    )
    .await?;
    sqlx::query(
        "INSERT INTO inventory_movements (
            id, branch_id, product_id, movement_type, quantity_delta, quantity_after,
            order_id, order_item_id, origin_event_id, created_by, created_at
         ) VALUES (?, ?, ?, 'sale_void', ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(branch_id)
    .bind(&product_id)
    .bind(quantity)
    .bind(after)
    .bind(&order_id)
    .bind(item_id)
    .bind(&out.event_id)
    .bind(user_id)
    .bind(&now)
    .execute(&mut *tx)
    .await?;
    insert_audit(
        &mut *tx,
        branch_id,
        user_id,
        device_id,
        "order.item_voided",
        "order_item",
        item_id,
        None,
        Some(&payload),
    )
    .await?;
    Ok(payload)
}

pub async fn adjust(
    pool: &SqlitePool,
    branch_id: &str,
    device_id: &str,
    product_id: &str,
    movement_type: &str,
    quantity_delta: i64,
    reason: &str,
    user_id: &str,
) -> AppResult<serde_json::Value> {
    if reason.trim().is_empty() {
        return Err(AppError::domain("adjustment requires a reason"));
    }
    match movement_type {
        "adjustment_in" | "adjustment_out" | "damaged" | "expired" | "opening" => {}
        _ => return Err(AppError::domain("invalid movement type")),
    }
    let mut tx = pool.begin().await?;
    let stock_row: Option<(i64, i64)> = sqlx::query_as(
        "SELECT quantity_on_hand, version FROM inventory_balances WHERE branch_id = ? AND product_id = ?",
    )
    .bind(branch_id)
    .bind(product_id)
    .fetch_optional(&mut *tx)
    .await?;
    let now = Utc::now().to_rfc3339();
    let after = match stock_row {
        Some((stock, version)) => {
            let after = stock + quantity_delta;
            if after < 0 {
                return Err(AppError::Conflict(
                    "inventory cannot become negative".into(),
                ));
            }
            apply_stock_cas(&mut tx, branch_id, product_id, stock, version, after, &now).await?;
            after
        }
        None => {
            if quantity_delta < 0 {
                return Err(AppError::Conflict(
                    "inventory cannot become negative".into(),
                ));
            }
            sqlx::query(
                "INSERT INTO inventory_balances (branch_id, product_id, quantity_on_hand, version, updated_at)
                 VALUES (?, ?, ?, 1, ?)",
            )
            .bind(branch_id)
            .bind(product_id)
            .bind(quantity_delta)
            .bind(&now)
            .execute(&mut *tx)
            .await?;
            quantity_delta
        }
    };

    let movement_id = Uuid::new_v4().to_string();
    let payload = serde_json::json!({
        "movement_id": movement_id,
        "branch_id": branch_id,
        "product_id": product_id,
        "movement_type": movement_type,
        "quantity_delta": quantity_delta,
        "quantity_after": after,
        "reason": reason,
        "created_by": user_id
    });
    let out = outbox::enqueue(
        &mut tx,
        device_id,
        branch_id,
        "inventory.adjusted",
        &movement_id,
        payload.clone(),
    )
    .await?;
    sqlx::query(
        "INSERT INTO inventory_movements (
            id, branch_id, product_id, movement_type, quantity_delta, quantity_after,
            reason, origin_event_id, created_by, created_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&movement_id)
    .bind(branch_id)
    .bind(product_id)
    .bind(movement_type)
    .bind(quantity_delta)
    .bind(after)
    .bind(reason)
    .bind(&out.event_id)
    .bind(user_id)
    .bind(&now)
    .execute(&mut *tx)
    .await?;
    insert_audit(
        &mut tx,
        branch_id,
        user_id,
        device_id,
        "inventory.adjusted",
        "inventory",
        product_id,
        None,
        Some(&payload),
    )
    .await?;
    tx.commit().await?;
    Ok(payload)
}

pub async fn stock(pool: &SqlitePool, branch_id: &str, product_id: &str) -> AppResult<i64> {
    Ok(sqlx::query_scalar(
        "SELECT quantity_on_hand FROM inventory_balances WHERE branch_id = ? AND product_id = ?",
    )
    .bind(branch_id)
    .bind(product_id)
    .fetch_optional(pool)
    .await?
    .unwrap_or(0))
}

pub async fn ensure_balance_row(
    tx: &mut SqliteConnection,
    branch_id: &str,
    product_id: &str,
) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT OR IGNORE INTO inventory_balances (branch_id, product_id, quantity_on_hand, version, updated_at)
         VALUES (?, ?, 0, 0, ?)",
    )
    .bind(branch_id)
    .bind(product_id)
    .bind(now)
    .execute(tx)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dev;

    #[tokio::test]
    async fn stale_version_is_rejected() {
        let pool = crate::database::open_memory().await.unwrap();
        dev::seed_two_branches(&pool).await.unwrap();
        let now = Utc::now().to_rfc3339();
        let mut tx = pool.begin().await.unwrap();
        let err = apply_stock_cas(&mut tx, "b1", "p-coke", 50, 0, 49, &now).await;
        assert!(err.is_err(), "stale inventory version must be rejected");
        tx.rollback().await.unwrap();
        assert_eq!(stock(&pool, "b1", "p-coke").await.unwrap(), 50);
    }
}
