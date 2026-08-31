use chrono::Utc;
use sqlx::SqlitePool;
use uuid::Uuid;

use super::gaming::insert_audit;
use super::money;
use super::tax;
use crate::error::{AppError, AppResult};
use crate::sync::outbox;

const CASH_METHOD_ID: &str = "11111111-1111-1111-1111-111111111111";

pub async fn take_cash(
    pool: &SqlitePool,
    branch_id: &str,
    device_id: &str,
    order_id: &str,
    tendered_minor: i64,
    user_id: &str,
) -> AppResult<serde_json::Value> {
    if tendered_minor < 0 {
        return Err(AppError::domain("tendered amount invalid"));
    }
    let mut tx = pool.begin().await?;
    let order: Option<(String, i64, i64, i64, i64, i64, String)> = sqlx::query_as(
        "SELECT status, product_subtotal_minor, gaming_subtotal_minor, discount_minor, tax_minor, tax_rate_bps, order_type
         FROM orders WHERE id = ? AND branch_id = ?",
    )
    .bind(order_id)
    .bind(branch_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((status, product, gaming, discount, tax_minor, tax_rate_bps, order_type)) = order
    else {
        return Err(AppError::NotFound("order".into()));
    };
    tax::reject_negative_tax(tax_minor, tax_rate_bps)
        .map_err(|_| AppError::domain("negative tax rejected"))?;
    if status == "paid" {
        return Err(AppError::Conflict("order already paid".into()));
    }
    if status == "void" {
        return Err(AppError::Conflict("void order cannot be paid".into()));
    }

    if order_type == "gaming" {
        let sess: Option<String> =
            sqlx::query_scalar("SELECT status FROM gaming_sessions WHERE order_id = ?")
                .bind(order_id)
                .fetch_optional(&mut *tx)
                .await?;
        if let Some(s) = sess {
            if s == "active" {
                return Err(AppError::Conflict("stop the session before payment".into()));
            }
        }
    }

    let subtotal =
        tax::subtotal(product, gaming).map_err(|_| AppError::domain("subtotal overflow"))?;
    let due = tax::total(subtotal, tax_minor, discount)
        .map_err(|_| AppError::domain("total overflow"))?;
    let change = money::change(tendered_minor, due)
        .map_err(|_| AppError::domain("cash tendered is less than amount due"))?;
    let payment_id = Uuid::new_v4().to_string();
    let now = Utc::now();
    let now_s = now.to_rfc3339();
    let day = now.format("%Y%m%d");
    // receipt_number is globally unique, so the counter has to be per branch and
    // per day and must never reuse a number. Counting the orders that hold a
    // receipt did both wrong: a repaid order retires one number and takes
    // another, after which the count points back at a number already in use, and
    // two branches paying on the same day both produced the same "B-<day>-0001".
    // The high-water row covers what the orders table cannot see: a restored
    // backup is missing orders the cloud still holds, and re-issuing one of those
    // numbers gets order.paid refused with a 409 for good.
    let branch_code: String = sqlx::query_scalar("SELECT code FROM branches WHERE id = ?")
        .bind(branch_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::NotFound("branch".into()))?;
    let prefix = format!("{branch_code}-{day}-");
    let seq: i64 = sqlx::query_scalar(
        "SELECT MAX(
                  (SELECT COALESCE(MAX(CAST(substr(receipt_number, ?) AS INTEGER)), 0)
                   FROM orders
                   WHERE branch_id = ? AND receipt_number LIKE ?),
                  (SELECT COALESCE(MAX(last_sequence), 0)
                   FROM receipt_high_water
                   WHERE branch_id = ? AND prefix = ?)
                )",
    )
    .bind(prefix.chars().count() as i64 + 1)
    .bind(branch_id)
    .bind(format!("{prefix}%"))
    .bind(branch_id)
    .bind(&prefix)
    .fetch_one(&mut *tx)
    .await?;
    let receipt_number = format!("{prefix}{:04}", seq + 1);

    let receipt_snapshot = serde_json::json!({
        "receipt_number": receipt_number,
        "order_id": order_id,
        "branch_id": branch_id,
        "paid_at": now_s,
        "product_subtotal_minor": product,
        "gaming_subtotal_minor": gaming,
        "subtotal_minor": subtotal,
        "discount_minor": discount,
        "tax_minor": tax_minor,
        "tax_rate_bps": tax_rate_bps,
        "total_minor": due,
        "amount_tendered_minor": tendered_minor,
        "amount_applied_minor": due,
        "change_minor": change,
        "currency_code": "EGP",
        "payment_method": "cash"
    });

    let payload_pay = serde_json::json!({
        "payment_id": payment_id,
        "order_id": order_id,
        "branch_id": branch_id,
        "payment_method_id": CASH_METHOD_ID,
        "amount_due_minor": due,
        "amount_tendered_minor": tendered_minor,
        "amount_applied_minor": due,
        "change_minor": change,
        "cashier_id": user_id,
        "paid_at": now_s
    });
    let out_pay = outbox::enqueue(
        &mut tx,
        device_id,
        branch_id,
        "payment.captured",
        &payment_id,
        payload_pay.clone(),
    )
    .await?;

    sqlx::query(
        "INSERT INTO payments (
            id, branch_id, order_id, payment_method_id, payment_type,
            amount_due_minor, amount_tendered_minor, amount_applied_minor, change_minor,
            status, cashier_id, paid_at, origin_event_id
         ) VALUES (?, ?, ?, ?, 'sale', ?, ?, ?, ?, 'captured', ?, ?, ?)",
    )
    .bind(&payment_id)
    .bind(branch_id)
    .bind(order_id)
    .bind(CASH_METHOD_ID)
    .bind(due)
    .bind(tendered_minor)
    .bind(due)
    .bind(change)
    .bind(user_id)
    .bind(&now_s)
    .bind(&out_pay.event_id)
    .execute(&mut *tx)
    .await?;

    let payload_order = serde_json::json!({
        "order_id": order_id,
        "branch_id": branch_id,
        "payment_id": payment_id,
        "total_minor": due,
        "amount_tendered_minor": tendered_minor,
        "amount_applied_minor": due,
        "change_minor": change,
        "currency_code": "EGP",
        "receipt_number": receipt_number,
        "receipt_snapshot": receipt_snapshot,
        "closed_by": user_id,
        "closed_at": now_s,
        "cashier_id": user_id,
        "paid_at": now_s,
        "payment_method_id": CASH_METHOD_ID,
        "amount_due_minor": due,
        "subtotal_minor": subtotal,
        "tax_minor": tax_minor,
        "tax_rate_bps": tax_rate_bps,
        "discount_minor": discount
    });
    outbox::enqueue(
        &mut tx,
        device_id,
        branch_id,
        "order.paid",
        order_id,
        payload_order.clone(),
    )
    .await?;

    sqlx::query(
        "UPDATE orders SET
            status = 'paid',
            subtotal_minor = ?,
            total_minor = ?,
            amount_paid_minor = ?,
            change_minor = ?,
            receipt_number = ?,
            receipt_snapshot = ?,
            closed_by = ?,
            closed_at = ?
         WHERE id = ? AND status <> 'paid'",
    )
    .bind(subtotal)
    .bind(due)
    .bind(due)
    .bind(change)
    .bind(&receipt_number)
    .bind(receipt_snapshot.to_string())
    .bind(user_id)
    .bind(&now_s)
    .bind(order_id)
    .execute(&mut *tx)
    .await?;

    insert_audit(
        &mut tx,
        branch_id,
        user_id,
        device_id,
        "order.paid",
        "order",
        order_id,
        None,
        Some(&payload_order),
    )
    .await?;
    tx.commit().await?;
    Ok(payload_order)
}

pub async fn reverse_payment(
    pool: &SqlitePool,
    branch_id: &str,
    device_id: &str,
    order_id: &str,
    user_id: &str,
    reason: &str,
) -> AppResult<serde_json::Value> {
    if reason.trim().is_empty() {
        return Err(AppError::domain("reversal requires a reason"));
    }
    let mut tx = pool.begin().await?;
    let status: String =
        sqlx::query_scalar("SELECT status FROM orders WHERE id = ? AND branch_id = ?")
            .bind(order_id)
            .bind(branch_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| AppError::NotFound("order".into()))?;
    if status != "paid" {
        return Err(AppError::Conflict(
            "only paid orders can be reversed".into(),
        ));
    }

    let parent: (String, i64) = sqlx::query_as(
        "SELECT id, amount_applied_minor FROM payments
         WHERE order_id = ? AND payment_type = 'sale' AND status = 'captured'",
    )
    .bind(order_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound("captured payment".into()))?;

    let reversal_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let payload = serde_json::json!({
        "payment_id": reversal_id,
        "parent_payment_id": parent.0,
        "order_id": order_id,
        "branch_id": branch_id,
        "amount_applied_minor": parent.1,
        "reversed_by": user_id,
        "reason": reason
    });
    let out = outbox::enqueue(
        &mut tx,
        device_id,
        branch_id,
        "payment.reversed",
        &reversal_id,
        payload.clone(),
    )
    .await?;

    sqlx::query("UPDATE payments SET status = 'reversed' WHERE id = ?")
        .bind(&parent.0)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "INSERT INTO payments (
            id, branch_id, order_id, payment_method_id, payment_type,
            amount_due_minor, amount_tendered_minor, amount_applied_minor, change_minor,
            status, parent_payment_id, cashier_id, paid_at, origin_event_id, reference
         ) VALUES (?, ?, ?, ?, 'reversal', ?, 0, ?, 0, 'reversed', ?, ?, ?, ?, ?)",
    )
    .bind(&reversal_id)
    .bind(branch_id)
    .bind(order_id)
    .bind(CASH_METHOD_ID)
    .bind(parent.1)
    .bind(parent.1)
    .bind(&parent.0)
    .bind(user_id)
    .bind(&now)
    .bind(&out.event_id)
    .bind(reason)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        "UPDATE orders SET
            status = 'checkout_pending',
            amount_paid_minor = 0,
            change_minor = 0,
            closed_by = NULL,
            closed_at = NULL
         WHERE id = ?",
    )
    .bind(order_id)
    .execute(&mut *tx)
    .await?;

    insert_audit(
        &mut tx,
        branch_id,
        user_id,
        device_id,
        "payment.reversed",
        "payment",
        &parent.0,
        None,
        Some(&payload),
    )
    .await?;
    tx.commit().await?;
    Ok(payload)
}
