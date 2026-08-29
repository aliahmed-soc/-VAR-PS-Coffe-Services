use std::sync::Arc;

use sqlx::SqlitePool;

use crate::auth::session::SessionStore;
use crate::sync::engine::SyncEngine;

pub struct AppState {
    pub db: SqlitePool,
    pub sessions: SessionStore,
    pub sync: Arc<SyncEngine>,
    pub device_id: String,
    pub app_data_dir: std::path::PathBuf,
}
