use std::sync::Arc;
use std::time::Duration;

use sqlx::SqlitePool;
use tokio::sync::Notify;

use super::{outbox, transport};
use crate::auth::session::SessionStore;
use crate::auth::supabase_auth;
use crate::error::{AppError, AppResult};

fn is_unauthorized(err: &AppError) -> bool {
    let msg = err.to_string();
    msg.contains("401") || msg.contains("unauthorized") || msg.contains("jwt expired")
}

async fn refresh_session(sessions: &SessionStore, cfg: &transport::SupabaseConfig) -> bool {
    let Some(refresh_token) = sessions.refresh_token() else {
        return false;
    };
    match supabase_auth::refresh(cfg, &refresh_token).await {
        Ok(tokens) => {
            sessions.update_tokens(tokens.access_token, tokens.refresh_token);
            true
        }
        Err(_) => {
            tracing::warn!("token refresh failed");
            false
        }
    }
}

pub struct SyncEngine {
    pool: SqlitePool,
    wake: Arc<Notify>,
}

impl SyncEngine {
    pub fn start(pool: SqlitePool, sessions: SessionStore) -> Arc<Self> {
        let engine = Arc::new(Self {
            pool,
            wake: Arc::new(Notify::new()),
        });
        let worker = engine.clone();
        tokio::spawn(async move {
            loop {
                if let Err(err) = worker.tick(&sessions).await {
                    tracing::warn!("sync tick failed: {err}");
                }
                tokio::select! {
                    _ = worker.wake.notified() => {}
                    _ = tokio::time::sleep(Duration::from_secs(5)) => {}
                }
            }
        });
        engine
    }

    pub fn notify(&self) {
        self.wake.notify_one();
    }

    async fn tick(&self, sessions: &SessionStore) -> AppResult<()> {
        let recon: i64 = sqlx::query_scalar(
            "SELECT restore_reconciliation_required FROM sync_state WHERE id = 1",
        )
        .fetch_one(&self.pool)
        .await
        .unwrap_or(0);
        if recon == 1 {
            if let (Some(cfg), Some(token), Some(branch)) = (
                transport::env_config(),
                sessions.access_token(),
                sessions.branch_id(),
            ) {
                let after = pull_cursor(&self.pool).await;
                match transport::pull_branch_since(&cfg, &token, &branch, &after).await {
                    Ok(snapshot) => {
                        apply_pull_snapshot(&self.pool, &snapshot).await?;
                    }
                    Err(e) if is_unauthorized(&e) && refresh_session(sessions, &cfg).await => {
                        if let Some(token) = sessions.access_token() {
                            match transport::pull_branch_since(&cfg, &token, &branch, &after).await
                            {
                                Ok(snapshot) => {
                                    apply_pull_snapshot(&self.pool, &snapshot).await?;
                                }
                                Err(retry) => {
                                    tracing::warn!("pull-before-push blocked after refresh");
                                    let _ = retry;
                                    return Ok(());
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("pull-before-push blocked: {e}");
                        return Ok(());
                    }
                }
            } else {
                return Ok(());
            }
        }

        let Some(cfg) = transport::env_config() else {
            return Ok(());
        };
        let Some(token) = sessions.access_token() else {
            return Ok(());
        };

        let pending = outbox::pending(&self.pool, 20).await?;
        for row in pending {
            if !outbox::mark_sending(&self.pool, &row.event_id).await? {
                continue;
            }
            let payload: serde_json::Value = serde_json::from_str(&row.payload_json)?;
            let req = transport::ApplyRequest {
                p_event_id: row.event_id.clone(),
                p_branch_id: row.branch_id.clone(),
                p_device_id: row.device_id.clone(),
                p_local_sequence: row.sequence,
                p_event_type: row.event_type.clone(),
                p_payload: payload,
                p_payload_hash: row.payload_hash.clone(),
            };
            let token = sessions.access_token().unwrap_or_else(|| token.clone());
            let applied = match transport::apply_domain_event(&cfg, &token, &req).await {
                Ok(result) => Ok(result),
                Err(e) if is_unauthorized(&e) && refresh_session(sessions, &cfg).await => {
                    let retry_token = sessions.access_token().unwrap_or(token);
                    transport::apply_domain_event(&cfg, &retry_token, &req).await
                }
                Err(e) => Err(e),
            };
            match applied {
                Ok(result)
                    if result.status == "applied" || result.status == "already_processed" =>
                {
                    outbox::mark_synced(&self.pool, &row.event_id).await?;
                }
                Ok(other) => {
                    outbox::mark_retry(
                        &self.pool,
                        &row.event_id,
                        &format!("{:?}", other.status),
                        row.attempt_count,
                    )
                    .await?;
                    break;
                }
                Err(e) => {
                    let msg = e.to_string();
                    if outbox::is_terminal_sync_error(&msg) {
                        outbox::mark_dead(&self.pool, &row.event_id, &msg).await?;
                    } else {
                        outbox::mark_retry(&self.pool, &row.event_id, &msg, row.attempt_count)
                            .await?;
                    }
                    if msg.contains("sequence_gap") {
                        break;
                    }
                }
            }
        }
        Ok(())
    }
}

pub async fn pull_cursor(pool: &SqlitePool) -> String {
    sqlx::query_scalar::<_, Option<String>>(
        "SELECT last_successful_pull_at FROM sync_state WHERE id = 1",
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
    .flatten()
    .filter(|s| !s.is_empty())
    .unwrap_or_else(|| "1970-01-01T00:00:00Z".into())
}

/// Moves this device's local counter past every sequence the cloud has already
/// accepted from it.
///
/// A restored backup can be older than what the cloud holds, which rewinds the
/// counter. The cloud demands exactly `last_applied + 1`, so from then on every
/// event dies with `sequence_gap` and the till can never push again. Only ever
/// forward: in ordinary operation the local counter is already ahead and this is
/// a no-op.
async fn fast_forward_sequences(pool: &SqlitePool, snapshot: &serde_json::Value) -> AppResult<()> {
    let Some(receipts) = snapshot.get("sync_receipts").and_then(|v| v.as_array()) else {
        return Ok(());
    };
    let mut highest: std::collections::HashMap<&str, i64> = std::collections::HashMap::new();
    for receipt in receipts {
        let Some(device) = receipt.get("device_id").and_then(|v| v.as_str()) else {
            continue;
        };
        let Some(sequence) = receipt.get("local_sequence").and_then(|v| v.as_i64()) else {
            continue;
        };
        let slot = highest.entry(device).or_insert(sequence);
        if sequence > *slot {
            *slot = sequence;
        }
    }
    for (device, sequence) in highest {
        sqlx::query(
            "UPDATE device_sequence SET next_sequence = ?
             WHERE device_id = ? AND next_sequence <= ?",
        )
        .bind(sequence + 1)
        .bind(device)
        .bind(sequence)
        .execute(pool)
        .await?;
    }
    Ok(())
}

pub async fn apply_pull_snapshot(pool: &SqlitePool, snapshot: &serde_json::Value) -> AppResult<u64> {
    let marked = outbox::reconcile_from_pull(pool, snapshot).await?;
    fast_forward_sequences(pool, snapshot).await?;
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE sync_state SET restore_reconciliation_required = 0, last_successful_pull_at = ?, updated_at = ? WHERE id = 1",
    )
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(marked)
}

pub async fn status(pool: &SqlitePool) -> AppResult<serde_json::Value> {
    let pending: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sync_outbox WHERE sync_status IN ('pending','failed','sending')",
    )
    .fetch_one(pool)
    .await?;
    let failed: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sync_outbox WHERE sync_status = 'failed' AND attempt_count >= 4",
    )
    .fetch_one(pool)
    .await?;
    let recon: i64 =
        sqlx::query_scalar("SELECT restore_reconciliation_required FROM sync_state WHERE id = 1")
            .fetch_one(pool)
            .await
            .unwrap_or(0);
    let last: Option<String> =
        sqlx::query_scalar("SELECT last_successful_push_at FROM sync_state WHERE id = 1")
            .fetch_optional(pool)
            .await?
            .flatten();
    let label = if recon == 1 {
        "RECONCILE REQUIRED".to_string()
    } else if failed > 0 {
        format!("SYNC ERROR • {failed} ITEMS NEED ATTENTION")
    } else if pending > 0 {
        format!("OFFLINE • {pending} UNSYNCED")
    } else {
        "ONLINE • SYNCED".to_string()
    };
    Ok(serde_json::json!({
        "label": label,
        "pending": pending,
        "failed": failed,
        "restore_reconciliation_required": recon == 1,
        "last_successful_push_at": last
    }))
}
