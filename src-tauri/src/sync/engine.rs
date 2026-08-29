use std::sync::Arc;
use std::time::Duration;

use sqlx::SqlitePool;
use tokio::sync::Notify;

use super::{outbox, transport};
use crate::auth::session::SessionStore;
use crate::error::AppResult;

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
            if let (Some(cfg), Some(token), Some(branch)) =
                (transport::env_config(), sessions.access_token(), sessions.branch_id())
            {
                let after = "1970-01-01T00:00:00Z";
                match transport::pull_branch_since(&cfg, &token, &branch, after).await {
                    Ok(_) => {
                        let now = chrono::Utc::now().to_rfc3339();
                        sqlx::query(
                            "UPDATE sync_state SET restore_reconciliation_required = 0, last_successful_pull_at = ?, updated_at = ? WHERE id = 1",
                        )
                        .bind(&now)
                        .bind(&now)
                        .execute(&self.pool)
                        .await?;
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
            match transport::apply_domain_event(&cfg, &token, &req).await {
                Ok(result) if result.status == "applied" || result.status == "already_processed" => {
                    outbox::mark_synced(&self.pool, &row.event_id).await?;
                }
                Ok(other) => {
                    outbox::mark_retry(&self.pool, &row.event_id, &format!("{:?}", other.status), row.attempt_count).await?;
                    break;
                }
                Err(e) => {
                    let msg = e.to_string();
                    outbox::mark_retry(&self.pool, &row.event_id, &msg, row.attempt_count).await?;
                    if msg.contains("sequence_gap") {
                        break;
                    }
                }
            }
        }
        Ok(())
    }
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
    let recon: i64 = sqlx::query_scalar(
        "SELECT restore_reconciliation_required FROM sync_state WHERE id = 1",
    )
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
