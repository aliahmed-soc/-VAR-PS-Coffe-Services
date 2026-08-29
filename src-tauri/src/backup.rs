use std::path::{Path, PathBuf};

use chrono::Utc;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::error::{AppError, AppResult};

pub const RETAINED_BACKUPS: usize = 14;

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
    let pruned = prune_backups(dest_dir).unwrap_or(0);
    Ok(serde_json::json!({
        "path": dest_s,
        "verified": true,
        "backup_id": Uuid::new_v4().to_string(),
        "retain": RETAINED_BACKUPS,
        "pruned": pruned
    }))
}

/// Keep the newest timestamped backups; delete older `.sqlite` files.
pub fn prune_backups(dest_dir: &Path) -> std::io::Result<usize> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dest_dir)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("sqlite"))
        .collect();
    files.sort_by(|a, b| b.file_name().cmp(&a.file_name()));
    let mut pruned = 0;
    for stale in files.into_iter().skip(RETAINED_BACKUPS) {
        if std::fs::remove_file(&stale).is_ok() {
            pruned += 1;
        }
    }
    Ok(pruned)
}

pub async fn stage_restore(
    backup_path: &PathBuf,
    live_db_path: &PathBuf,
) -> AppResult<serde_json::Value> {
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

pub fn list_backups(dest_dir: &PathBuf) -> AppResult<serde_json::Value> {
    let mut items = Vec::new();
    if dest_dir.exists() {
        for entry in std::fs::read_dir(dest_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("sqlite") {
                continue;
            }
            let meta = entry.metadata()?;
            items.push(serde_json::json!({
                "path": path.to_string_lossy(),
                "name": path.file_name().map(|n| n.to_string_lossy()).unwrap_or_default(),
                "bytes": meta.len()
            }));
        }
    }
    items.sort_by(|a, b| b["name"].as_str().cmp(&a["name"].as_str()));
    Ok(serde_json::json!({ "backups": items }))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prune_keeps_newest_fourteen() {
        let dir = std::env::temp_dir().join(format!("psc-backup-prune-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        for i in 0..16 {
            std::fs::write(dir.join(format!("branch-20260829T{i:06}Z.sqlite")), b"x").unwrap();
        }
        let pruned = prune_backups(&dir).unwrap();
        assert_eq!(pruned, 2);
        let left: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        assert_eq!(left.len(), RETAINED_BACKUPS);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
