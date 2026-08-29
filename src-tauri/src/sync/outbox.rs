use chrono::Utc;
use sqlx::{SqliteConnection, SqlitePool};
use uuid::Uuid;

use crate::domain::events::{self, aggregate_for, payload_hash, validate_payload};
use crate::error::{AppError, AppResult};
use crate::sync::sequence;

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct OutboxRow {
    pub event_id: String,
    pub sequence: i64,
    pub event_type: String,
    pub aggregate_type: String,
    pub aggregate_id: String,
    pub branch_id: String,
    pub device_id: String,
    pub payload_json: String,
    pub payload_hash: String,
    pub created_at: String,
    pub attempt_count: i64,
    pub next_attempt_at: String,
    pub last_error: Option<String>,
    pub sync_status: String,
}

pub async fn enqueue(
    tx: &mut SqliteConnection,
    device_id: &str,
    branch_id: &str,
    event_type: &str,
    aggregate_id: &str,
    payload: serde_json::Value,
) -> AppResult<OutboxRow> {
    if !events::is_known_event(event_type) {
        return Err(AppError::domain(format!("unknown event {event_type}")));
    }
    validate_payload(event_type, &payload).map_err(AppError::domain)?;
    let event_id = Uuid::new_v4().to_string();
    let sequence = sequence::next_in_tx(tx, device_id).await?;
    let aggregate_type = aggregate_for(event_type).unwrap_or_else(|| "unknown".into());
    let hash = payload_hash(&payload);
    let now = Utc::now().to_rfc3339();
    let payload_json = serde_json::to_string(&payload)?;

    sqlx::query(
        "INSERT INTO sync_outbox (
            event_id, sequence, event_type, aggregate_type, aggregate_id,
            branch_id, device_id, payload_json, payload_hash, created_at,
            attempt_count, next_attempt_at, sync_status
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, ?, 'pending')",
    )
    .bind(&event_id)
    .bind(sequence)
    .bind(event_type)
    .bind(&aggregate_type)
    .bind(aggregate_id)
    .bind(branch_id)
    .bind(device_id)
    .bind(&payload_json)
    .bind(&hash)
    .bind(&now)
    .bind(&now)
    .execute(&mut *tx)
    .await?;

    sqlx::query("UPDATE sync_state SET pending_count = pending_count + 1, updated_at = ? WHERE id = 1")
        .bind(&now)
        .execute(&mut *tx)
        .await?;

    Ok(OutboxRow {
        event_id,
        sequence,
        event_type: event_type.into(),
        aggregate_type,
        aggregate_id: aggregate_id.into(),
        branch_id: branch_id.into(),
        device_id: device_id.into(),
        payload_json,
        payload_hash: hash,
        created_at: now.clone(),
        attempt_count: 0,
        next_attempt_at: now,
        last_error: None,
        sync_status: "pending".into(),
    })
}

pub async fn pending(pool: &SqlitePool, limit: i64) -> AppResult<Vec<OutboxRow>> {
    let now = Utc::now().to_rfc3339();
    let rows = sqlx::query_as::<_, OutboxRow>(
        "SELECT * FROM sync_outbox
         WHERE sync_status IN ('pending', 'failed')
           AND next_attempt_at <= ?
         ORDER BY sequence ASC
         LIMIT ?",
    )
    .bind(now)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub fn next_backoff_secs(attempt_count: i64) -> i64 {
    match attempt_count {
        0 | 1 => 5,
        2 => 15,
        3 => 30,
        4 => 60,
        n => (60 * (n - 3)).min(15 * 60),
    }
}

pub async fn mark_synced(pool: &SqlitePool, event_id: &str) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    sqlx::query("UPDATE sync_outbox SET sync_status = 'synced', last_error = NULL WHERE event_id = ?")
        .bind(event_id)
        .execute(pool)
        .await?;
    sqlx::query(
        "UPDATE sync_state SET pending_count = (
            SELECT COUNT(*) FROM sync_outbox WHERE sync_status IN ('pending','failed','sending')
         ), last_successful_push_at = ?, updated_at = ? WHERE id = 1",
    )
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn mark_retry(pool: &SqlitePool, event_id: &str, error: &str, attempt_count: i64) -> AppResult<()> {
    let delay = next_backoff_secs(attempt_count + 1);
    let next = (Utc::now() + chrono::Duration::seconds(delay)).to_rfc3339();
    sqlx::query(
        "UPDATE sync_outbox
         SET sync_status = 'failed', attempt_count = attempt_count + 1,
             next_attempt_at = ?, last_error = ?
         WHERE event_id = ?",
    )
    .bind(next)
    .bind(error)
    .bind(event_id)
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::next_backoff_secs;

    #[test]
    fn backoff_schedule() {
        assert_eq!(next_backoff_secs(1), 5);
        assert_eq!(next_backoff_secs(2), 15);
        assert_eq!(next_backoff_secs(3), 30);
        assert_eq!(next_backoff_secs(4), 60);
        assert!(next_backoff_secs(20) <= 15 * 60);
    }
}
