use chrono::Utc;
use sqlx::SqlitePool;
use uuid::Uuid;

use super::gaming::insert_audit;
use crate::error::{AppError, AppResult};
use crate::sync::outbox;

pub async fn open_pos_order(
    pool: &SqlitePool,
    branch_id: &str,
    device_id: &str,
    user_id: &str,
) -> AppResult<serde_json::Value> {
    let mut tx = pool.begin().await?;
    let order_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO orders (
            id, branch_id, order_type, status, currency_code, opened_by, opened_at,
            tax_minor, tax_rate_bps, origin_device_id
         ) VALUES (?, ?, 'pos', 'open', 'EGP', ?, ?, 0, 0, ?)",
    )
    .bind(&order_id)
    .bind(branch_id)
    .bind(user_id)
    .bind(&now)
    .bind(device_id)
    .execute(&mut *tx)
    .await?;
    let payload = serde_json::json!({
        "order_id": order_id,
        "branch_id": branch_id,
        "order_type": "pos",
        "opened_by": user_id,
        "opened_at": now,
        "currency_code": "EGP"
    });
    insert_audit(
        &mut tx,
        branch_id,
        user_id,
        device_id,
        "order.opened",
        "order",
        &order_id,
        None,
        Some(&payload),
    )
    .await?;
    outbox::enqueue(
        &mut tx,
        device_id,
        branch_id,
        "order.opened",
        &order_id,
        payload.clone(),
    )
    .await?;
    tx.commit().await?;
    Ok(payload)
}

#[derive(sqlx::FromRow)]
struct OrderRow {
    id: String,
    branch_id: String,
    order_type: String,
    status: String,
    product_subtotal_minor: i64,
    gaming_subtotal_minor: i64,
    subtotal_minor: i64,
    discount_minor: i64,
    tax_minor: i64,
    tax_rate_bps: i64,
    total_minor: i64,
    amount_paid_minor: i64,
    change_minor: i64,
    currency_code: String,
    receipt_number: Option<String>,
    receipt_snapshot: Option<String>,
    opened_at: String,
    closed_at: Option<String>,
}

pub async fn get_order(
    pool: &SqlitePool,
    branch_id: &str,
    order_id: &str,
) -> AppResult<serde_json::Value> {
    let row = sqlx::query_as::<_, OrderRow>(
        "SELECT id, branch_id, order_type, status, product_subtotal_minor, gaming_subtotal_minor,
                subtotal_minor, discount_minor, tax_minor, tax_rate_bps, total_minor, amount_paid_minor, change_minor,
                currency_code, receipt_number, receipt_snapshot, opened_at, closed_at
         FROM orders WHERE id = ? AND branch_id = ?",
    )
    .bind(order_id)
    .bind(branch_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("order".into()))?;

    let items: Vec<(String, String, i64, i64, i64, String)> = sqlx::query_as(
        "SELECT id, product_name_snapshot, quantity, unit_price_minor, line_total_minor, status
         FROM order_items WHERE order_id = ? ORDER BY added_at",
    )
    .bind(order_id)
    .fetch_all(pool)
    .await?;

    Ok(serde_json::json!({
        "id": row.id,
        "branch_id": row.branch_id,
        "order_type": row.order_type,
        "status": row.status,
        "product_subtotal_minor": row.product_subtotal_minor,
        "gaming_subtotal_minor": row.gaming_subtotal_minor,
        "subtotal_minor": row.subtotal_minor,
        "discount_minor": row.discount_minor,
        "tax_minor": row.tax_minor,
        "tax_rate_bps": row.tax_rate_bps,
        "total_minor": row.total_minor,
        "amount_paid_minor": row.amount_paid_minor,
        "change_minor": row.change_minor,
        "currency_code": row.currency_code,
        "receipt_number": row.receipt_number,
        "receipt_snapshot": row.receipt_snapshot,
        "opened_at": row.opened_at,
        "closed_at": row.closed_at,
        "items": items.into_iter().map(|i| serde_json::json!({
            "id": i.0,
            "name": i.1,
            "quantity": i.2,
            "unit_price_minor": i.3,
            "line_total_minor": i.4,
            "status": i.5
        })).collect::<Vec<_>>()
    }))
}

pub async fn void_open_order(
    pool: &SqlitePool,
    branch_id: &str,
    device_id: &str,
    order_id: &str,
    user_id: &str,
    reason: &str,
) -> AppResult<serde_json::Value> {
    let mut tx = pool.begin().await?;
    let status: String =
        sqlx::query_scalar("SELECT status FROM orders WHERE id = ? AND branch_id = ?")
            .bind(order_id)
            .bind(branch_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| AppError::NotFound("order".into()))?;
    if status == "paid" {
        return Err(AppError::Conflict(
            "paid order cannot become open or void without reversal".into(),
        ));
    }
    // A voided ticket sold nothing, so every line it still holds goes back on the
    // shelf. Voiding only the order left the lines 'active' and kept the units it
    // had already deducted, so each mistyped ticket quietly shrank stock. Each
    // line is retired through the same per-line path a cashier would use, which
    // credits the stock, writes the movement, and carries its own event, so the
    // cloud converges on the already-proven order.item_voided handler.
    let active: Vec<String> = sqlx::query_scalar(
        "SELECT id FROM order_items WHERE order_id = ? AND status = 'active' ORDER BY id",
    )
    .bind(order_id)
    .fetch_all(&mut *tx)
    .await?;
    for item_id in &active {
        crate::domain::inventory::void_item_in_tx(
            &mut tx, branch_id, device_id, item_id, user_id, reason,
        )
        .await?;
    }

    sqlx::query("UPDATE orders SET status = 'void' WHERE id = ?")
        .bind(order_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE gaming_sessions SET status = 'void' WHERE order_id = ? AND status IN ('active','stopped')")
        .bind(order_id)
        .execute(&mut *tx)
        .await?;
    let payload = serde_json::json!({
        "order_id": order_id,
        "branch_id": branch_id,
        "voided_by": user_id,
        "reason": reason
    });
    insert_audit(
        &mut tx,
        branch_id,
        user_id,
        device_id,
        "order.voided",
        "order",
        order_id,
        None,
        Some(&payload),
    )
    .await?;
    outbox::enqueue(
        &mut tx,
        device_id,
        branch_id,
        "order.voided",
        order_id,
        payload.clone(),
    )
    .await?;
    tx.commit().await?;
    Ok(payload)
}

pub fn canonical_total(product: i64, gaming: i64, discount: i64, tax_minor: i64) -> i64 {
    crate::domain::tax::canonical_total(product, gaming, tax_minor, discount).unwrap_or(0)
}
