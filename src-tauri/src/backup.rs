use std::path::PathBuf;

use chrono::Utc;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::error::{AppError, AppResult};

pub async fn backup_now(pool: &SqlitePool, dest_dir: &PathBuf) -> AppResult<serde_json::Value> {
    std::fs::create_dir_all(dest_dir)?;
    let stamp = Utc::now().format("%Y%m%dT%H%M%SZ");
    let dest = dest_dir.join(format!("branch-{stamp}.sqlite"));
    let dest_s = dest.to_string_lossy().replace('\\', "/");
    sqlx::query(&format!("VACUUM INTO '{dest_s}'"))
        .execute(pool)
        .await
        .map_err(|e| AppError::Other(format!("backup failed: {e}")))?;

    let check: String = {
        let url = format!("sqlite://{dest_s}");
        let probe = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect(&url)
            .await?;
        let ok: String = sqlx::query_scalar("PRAGMA integrity_check")
            .fetch_one(&probe)
            .await?;
        probe.close().await;
        ok
    };
    if check != "ok" {
        return Err(AppError::Other(format!("backup integrity failed: {check}")));
    }
    Ok(serde_json::json!({
        "path": dest_s,
        "verified": true,
        "backup_id": Uuid::new_v4().to_string()
    }))
}

pub async fn stage_restore(backup_path: &PathBuf, live_db_path: &PathBuf) -> AppResult<serde_json::Value> {
    let dest_s = backup_path.to_string_lossy().replace('\\', "/");
    let url = format!("sqlite://{dest_s}");
    let probe = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await
        .map_err(|e| AppError::Other(format!("cannot open backup: {e}")))?;
    let check: String = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_one(&probe)
        .await?;
    probe.close().await;
    if check != "ok" {
        return Err(AppError::Other(format!("backup integrity failed: {check}")));
    }
    let staged = live_db_path.with_extension("sqlite.restore");
    std::fs::copy(backup_path, &staged)?;
    Ok(serde_json::json!({
        "staged": staged.to_string_lossy(),
        "restart_required": true,
        "pull_before_push": true
    }))
}

pub async fn mark_restore_reconcile(pool: &SqlitePool) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE sync_state SET restore_reconciliation_required = 1, updated_at = ? WHERE id = 1",
    )
    .bind(now)
    .execute(pool)
    .await?;
    Ok(())
}
