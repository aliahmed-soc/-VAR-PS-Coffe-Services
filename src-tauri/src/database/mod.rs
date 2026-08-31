pub mod migrate;

use std::path::{Path, PathBuf};

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::SqlitePool;
use std::str::FromStr;

use crate::error::AppResult;

pub async fn open_pool(path: &PathBuf) -> AppResult<SqlitePool> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    apply_pending_restore(path)?;
    let url = format!(
        "sqlite://{}?mode=rwc",
        path.to_string_lossy().replace('\\', "/")
    );
    let options = SqliteConnectOptions::from_str(&url)?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .foreign_keys(true)
        .busy_timeout(std::time::Duration::from_secs(5));

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;

    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await?;
    sqlx::query("PRAGMA busy_timeout = 5000")
        .execute(&pool)
        .await?;
    migrate::apply(&pool).await?;
    ensure_sync_state(&pool).await?;
    crate::sync::outbox::recover_stale_sending(&pool).await?;
    if path.with_extension("sqlite.pre-restore").exists() {
        crate::backup::mark_restore_reconcile(&pool).await?;
    }
    Ok(pool)
}

pub async fn open_memory() -> AppResult<SqlitePool> {
    let options = SqliteConnectOptions::from_str("sqlite::memory:")?
        .journal_mode(SqliteJournalMode::Wal)
        .foreign_keys(true)
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await?;
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await?;
    migrate::apply(&pool).await?;
    ensure_sync_state(&pool).await?;
    crate::sync::outbox::recover_stale_sending(&pool).await?;
    Ok(pool)
}

async fn ensure_sync_state(pool: &SqlitePool) -> AppResult<()> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT OR IGNORE INTO sync_state (
            id, cloud_connectivity, restore_reconciliation_required, pending_count, updated_at
         ) VALUES (1, 'unknown', 0, 0, ?)",
    )
    .bind(now)
    .execute(pool)
    .await?;
    Ok(())
}

/// SQLite's sidecar naming: `branch.sqlite` plus `-wal` is `branch.sqlite-wal`.
fn sidecar(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(suffix);
    path.with_file_name(name)
}

fn apply_pending_restore(path: &PathBuf) -> AppResult<()> {
    let restore = path.with_extension("sqlite.restore");
    if !restore.exists() {
        return Ok(());
    }
    if path.exists() {
        let pre = path.with_extension("sqlite.pre-restore");
        let _ = std::fs::remove_file(&pre);
        std::fs::rename(path, &pre)?;
        // The live -wal/-shm belong to the database that just moved aside. Left
        // behind under the old name, SQLite recovers those frames into the
        // restored file, because a foreign WAL is only checked against its own
        // frame checksums. That is how a restored till came up on a corrupt
        // index and then refused every sale with "database disk image is
        // malformed". They travel with the copy they describe instead, which
        // also keeps the pre-restore copy complete.
        for suffix in ["-wal", "-shm"] {
            let from = sidecar(path, suffix);
            if !from.exists() {
                continue;
            }
            let to = sidecar(&pre, suffix);
            let _ = std::fs::remove_file(&to);
            std::fs::rename(&from, &to)?;
        }
    }
    std::fs::rename(&restore, path)?;
    Ok(())
}

pub fn default_db_path() -> PathBuf {
    let base = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("playstation-cafe").join("branch.sqlite")
}
