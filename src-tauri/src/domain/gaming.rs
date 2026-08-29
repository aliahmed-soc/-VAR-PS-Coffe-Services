use std::collections::HashMap;
use std::sync::Mutex;

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{SqliteConnection, SqlitePool};
use uuid::Uuid;

use super::clock;
use super::events;
use super::pricing::{self, PricingSnapshot};
use crate::error::{AppError, AppResult};
use crate::sync::outbox;

static LAST_OBSERVED: Mutex<HashMap<String, DateTime<Utc>>> = Mutex::new(HashMap::new());

fn observe_clock(session_id: &str, started: DateTime<Utc>, now: DateTime<Utc>) -> bool {
    let mut map = LAST_OBSERVED.lock().unwrap_or_else(|e| e.into_inner());
    let previous = map.get(session_id).copied();
    let anomaly = clock::session_clock_anomaly(started, previous, now);
    map.insert(session_id.to_string(), now);
    anomaly
}

fn forget_clock(session_id: &str) {
    LAST_OBSERVED
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(session_id);
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct StationRow {
    pub id: String,
    pub branch_id: String,
    pub code: String,
    pub display_name: String,
    pub sort_order: i64,
    pub is_active: i64,
    pub session_id: Option<String>,
    pub session_status: Option<String>,
    pub started_at: Option<String>,
    pub order_id: Option<String>,
    pub pricing_snapshot: Option<String>,
}

pub async fn list_stations(pool: &SqlitePool, branch_id: &str) -> AppResult<Vec<StationRow>> {
    let rows = sqlx::query_as::<_, StationRow>(
        "SELECT s.id, s.branch_id, s.code, s.display_name, s.sort_order, s.is_active,
                g.id as session_id, g.status as session_status, g.started_at, g.order_id,
                g.pricing_snapshot
         FROM stations s
         LEFT JOIN gaming_sessions g
           ON g.id = (
             SELECT g2.id FROM gaming_sessions g2
             INNER JOIN orders o ON o.id = g2.order_id
             WHERE g2.station_id = s.id
               AND g2.branch_id = s.branch_id
               AND (
                 g2.status = 'active'
                 OR (g2.status = 'stopped' AND o.status IN ('open', 'checkout_pending'))
               )
             ORDER BY CASE g2.status WHEN 'active' THEN 0 ELSE 1 END, g2.started_at DESC
             LIMIT 1
           )
         WHERE s.branch_id = ?
         ORDER BY s.sort_order, s.code",
    )
    .bind(branch_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn start_session(
    pool: &SqlitePool,
    branch_id: &str,
    device_id: &str,
    station_id: &str,
    user_id: &str,
) -> AppResult<serde_json::Value> {
    let mut tx = pool.begin().await?;
    let occupied: Option<String> = sqlx::query_scalar(
        "SELECT id FROM gaming_sessions WHERE branch_id = ? AND station_id = ? AND status = 'active'",
    )
    .bind(branch_id)
    .bind(station_id)
    .fetch_optional(&mut *tx)
    .await?;
    if occupied.is_some() {
        return Err(AppError::Conflict("station already occupied".into()));
    }

    let rule: Option<(
        String,
        String,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        i64,
    )> = sqlx::query_as(
        "SELECT id, rule_type, rate_minor_per_hour, billing_increment_seconds,
                    base_duration_seconds, base_charge_minor, step_duration_seconds,
                    step_charge_minor, round_partial_step_up
             FROM pricing_rules
             WHERE branch_id = ? AND retired_at IS NULL
             ORDER BY effective_from DESC LIMIT 1",
    )
    .bind(branch_id)
    .fetch_optional(&mut *tx)
    .await?;

    let (rule_id, snapshot) = match rule {
        Some((id, rule_type, rate, inc, base_d, base_c, step_d, step_c, round_up)) => {
            let snap = PricingSnapshot {
                rule_type: if rule_type == "stepped" {
                    pricing::RuleType::Stepped
                } else {
                    pricing::RuleType::Linear
                },
                rate_minor_per_hour: rate,
                billing_increment_seconds: inc,
                base_duration_seconds: base_d,
                base_charge_minor: base_c,
                step_duration_seconds: step_d,
                step_charge_minor: step_c,
                round_partial_step_up: round_up != 0,
            };
            (id, snap)
        }
        None => {
            return Err(AppError::domain("no active pricing rule for branch"));
        }
    };

    let now = Utc::now();
    let now_s = now.to_rfc3339();
    let order_id = Uuid::new_v4().to_string();
    let session_id = Uuid::new_v4().to_string();
    let snap_json = serde_json::to_value(&snapshot)?;

    sqlx::query(
        "INSERT INTO orders (
            id, branch_id, order_type, status, currency_code, opened_by, opened_at,
            tax_minor, tax_rate_bps, origin_device_id
         ) VALUES (?, ?, 'gaming', 'open', 'EGP', ?, ?, 0, 0, ?)",
    )
    .bind(&order_id)
    .bind(branch_id)
    .bind(user_id)
    .bind(&now_s)
    .bind(device_id)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        "INSERT INTO gaming_sessions (
            id, branch_id, station_id, order_id, status, started_at,
            pricing_rule_id, pricing_snapshot, started_by
         ) VALUES (?, ?, ?, ?, 'active', ?, ?, ?, ?)",
    )
    .bind(&session_id)
    .bind(branch_id)
    .bind(station_id)
    .bind(&order_id)
    .bind(&now_s)
    .bind(&rule_id)
    .bind(snap_json.to_string())
    .bind(user_id)
    .execute(&mut *tx)
    .await?;

    insert_audit(
        &mut tx,
        branch_id,
        user_id,
        device_id,
        "session.started",
        "gaming_session",
        &session_id,
        None,
        Some(&snap_json),
    )
    .await?;

    let order_payload = serde_json::json!({
        "order_id": order_id,
        "branch_id": branch_id,
        "order_type": "gaming",
        "opened_by": user_id,
        "opened_at": now_s,
        "currency_code": "EGP"
    });
    outbox::enqueue(
        &mut tx,
        device_id,
        branch_id,
        "order.opened",
        &order_id,
        order_payload,
    )
    .await?;

    let payload = serde_json::json!({
        "session_id": session_id,
        "branch_id": branch_id,
        "station_id": station_id,
        "order_id": order_id,
        "started_at": now_s,
        "pricing_rule_id": rule_id,
        "pricing_snapshot": snap_json,
        "started_by": user_id
    });
    outbox::enqueue(
        &mut tx,
        device_id,
        branch_id,
        "session.started",
        &session_id,
        payload.clone(),
    )
    .await?;

    tx.commit().await?;
    Ok(payload)
}

pub async fn live_charge(pool: &SqlitePool, session_id: &str) -> AppResult<serde_json::Value> {
    let row: (String, String, String) = sqlx::query_as(
        "SELECT started_at, pricing_snapshot, status FROM gaming_sessions WHERE id = ?",
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("session".into()))?;

    let started = clock::parse_utc(&row.0).map_err(AppError::domain)?;
    let now = Utc::now();
    let anomaly = observe_clock(session_id, started, now);
    let snap: PricingSnapshot = serde_json::from_str(&row.1)?;
    let result = pricing::calculate(&snap, started.timestamp(), now.timestamp());
    Ok(serde_json::json!({
        "session_id": session_id,
        "status": row.2,
        "started_at": row.0,
        "duration_seconds": result.duration_seconds,
        "charge_minor": result.charge_minor,
        "clock_anomaly": anomaly,
    }))
}

pub async fn stop_session(
    pool: &SqlitePool,
    branch_id: &str,
    device_id: &str,
    session_id: &str,
    user_id: &str,
) -> AppResult<serde_json::Value> {
    let mut tx = pool.begin().await?;
    let row: Option<(String, String, String, String)> = sqlx::query_as(
        "SELECT status, started_at, pricing_snapshot, order_id FROM gaming_sessions WHERE id = ? AND branch_id = ?",
    )
    .bind(session_id)
    .bind(branch_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((status, started_at, snap_s, order_id)) = row else {
        return Err(AppError::NotFound("session".into()));
    };
    if status == "stopped" {
        let existing: (Option<i64>, Option<i64>) = sqlx::query_as(
            "SELECT duration_seconds, final_charge_minor FROM gaming_sessions WHERE id = ?",
        )
        .bind(session_id)
        .fetch_one(&mut *tx)
        .await?;
        return Ok(serde_json::json!({
            "session_id": session_id,
            "status": "stopped",
            "duration_seconds": existing.0,
            "final_charge_minor": existing.1,
            "idempotent": true
        }));
    }
    if status != "active" {
        return Err(AppError::Conflict(format!(
            "cannot stop session in {status}"
        )));
    }

    let started = clock::parse_utc(&started_at).map_err(AppError::domain)?;
    let now = Utc::now();
    let snap: PricingSnapshot = serde_json::from_str(&snap_s)?;
    let result = pricing::calculate(&snap, started.timestamp(), now.timestamp());
    let anomaly = observe_clock(session_id, started, now);
    forget_clock(session_id);
    let now_s = now.to_rfc3339();

    sqlx::query(
        "UPDATE gaming_sessions
         SET status = 'stopped', ended_at = ?, duration_seconds = ?,
             calculated_charge_minor = ?, final_charge_minor = ?, stopped_by = ?,
             clock_anomaly = ?
         WHERE id = ?",
    )
    .bind(&now_s)
    .bind(result.duration_seconds)
    .bind(result.charge_minor)
    .bind(result.charge_minor)
    .bind(user_id)
    .bind(if anomaly { 1 } else { 0 })
    .bind(session_id)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        "UPDATE orders
         SET gaming_subtotal_minor = ?,
             subtotal_minor = product_subtotal_minor + ?,
             total_minor = product_subtotal_minor + ? - discount_minor + tax_minor,
             status = 'checkout_pending'
         WHERE id = ?",
    )
    .bind(result.charge_minor)
    .bind(result.charge_minor)
    .bind(result.charge_minor)
    .bind(&order_id)
    .execute(&mut *tx)
    .await?;

    let payload = serde_json::json!({
        "session_id": session_id,
        "branch_id": branch_id,
        "ended_at": now_s,
        "duration_seconds": result.duration_seconds,
        "calculated_charge_minor": result.charge_minor,
        "final_charge_minor": result.charge_minor,
        "stopped_by": user_id
    });
    insert_audit(
        &mut tx,
        branch_id,
        user_id,
        device_id,
        "session.stopped",
        "gaming_session",
        session_id,
        None,
        Some(&payload),
    )
    .await?;
    outbox::enqueue(
        &mut tx,
        device_id,
        branch_id,
        "session.stopped",
        session_id,
        payload.clone(),
    )
    .await?;
    tx.commit().await?;
    Ok(payload)
}

pub async fn resume_session(
    pool: &SqlitePool,
    branch_id: &str,
    device_id: &str,
    session_id: &str,
    user_id: &str,
    reason: &str,
) -> AppResult<serde_json::Value> {
    let mut tx = pool.begin().await?;
    let row: Option<(String, String, String)> = sqlx::query_as(
        "SELECT status, order_id, station_id FROM gaming_sessions WHERE id = ? AND branch_id = ?",
    )
    .bind(session_id)
    .bind(branch_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((status, order_id, station_id)) = row else {
        return Err(AppError::NotFound("session".into()));
    };
    if status != "stopped" {
        return Err(AppError::Conflict(
            "resume only before payment on a stopped session".into(),
        ));
    }
    let order_status: String = sqlx::query_scalar("SELECT status FROM orders WHERE id = ?")
        .bind(&order_id)
        .fetch_one(&mut *tx)
        .await?;
    if order_status == "paid" {
        return Err(AppError::Conflict("paid order cannot resume".into()));
    }
    let occupied: Option<String> = sqlx::query_scalar(
        "SELECT id FROM gaming_sessions WHERE branch_id = ? AND station_id = ? AND status = 'active' AND id <> ?",
    )
    .bind(branch_id)
    .bind(&station_id)
    .bind(session_id)
    .fetch_optional(&mut *tx)
    .await?;
    if occupied.is_some() {
        return Err(AppError::Conflict(
            "station occupied by another session".into(),
        ));
    }

    sqlx::query(
        "UPDATE gaming_sessions
         SET status = 'active', ended_at = NULL, duration_seconds = NULL,
             calculated_charge_minor = NULL, final_charge_minor = NULL, stopped_by = NULL
         WHERE id = ?",
    )
    .bind(session_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE orders SET status = 'open', gaming_subtotal_minor = 0, subtotal_minor = product_subtotal_minor, total_minor = product_subtotal_minor - discount_minor + tax_minor WHERE id = ?")
        .bind(&order_id)
        .execute(&mut *tx)
        .await?;

    let payload = serde_json::json!({
        "session_id": session_id,
        "branch_id": branch_id,
        "resumed_by": user_id,
        "reason": reason
    });
    insert_audit(
        &mut tx,
        branch_id,
        user_id,
        device_id,
        "session.resumed",
        "gaming_session",
        session_id,
        None,
        Some(&payload),
    )
    .await?;
    outbox::enqueue(
        &mut tx,
        device_id,
        branch_id,
        "session.resumed",
        session_id,
        payload.clone(),
    )
    .await?;
    tx.commit().await?;
    Ok(payload)
}

pub(crate) async fn insert_audit(
    tx: &mut SqliteConnection,
    branch_id: &str,
    user_id: &str,
    device_id: &str,
    action: &str,
    entity_type: &str,
    entity_id: &str,
    previous: Option<&serde_json::Value>,
    new_data: Option<&serde_json::Value>,
) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO audit_logs (
            id, branch_id, user_id, device_id, action, entity_type, entity_id,
            previous_data, new_data, created_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(branch_id)
    .bind(user_id)
    .bind(device_id)
    .bind(action)
    .bind(entity_type)
    .bind(entity_id)
    .bind(previous.map(|v| v.to_string()))
    .bind(new_data.map(|v| v.to_string()))
    .bind(now)
    .execute(tx)
    .await?;
    let _ = events::is_known_event(action);
    Ok(())
}
