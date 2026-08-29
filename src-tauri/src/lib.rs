mod auth;
pub mod backup;
mod commands;
pub mod database;
pub mod dev;
mod device;
pub mod domain;
mod error;
pub mod reports;
mod state;
pub mod sync;

use std::sync::Arc;

use state::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter("playstation_cafe=info,sqlx=warn")
        .try_init()
        .ok();

    tauri::Builder::default()
        .setup(|app| {
            let handle = app.handle().clone();
            tauri::async_runtime::block_on(async move {
                let app_data = handle
                    .path()
                    .app_data_dir()
                    .unwrap_or_else(|_| std::path::PathBuf::from("./data"));
                std::fs::create_dir_all(&app_data)?;
                let db_path = app_data.join("branch.sqlite");
                let db = database::open_pool(&db_path).await?;
                let device_id = device::load_or_create(&app_data)?;
                let sessions = auth::session::SessionStore::new();
                let sync = sync::engine::SyncEngine::start(db.clone(), sessions.clone());
                handle.manage(AppState {
                    db,
                    sessions,
                    sync: Arc::clone(&sync),
                    device_id,
                    app_data_dir: app_data,
                });
                Ok::<(), Box<dyn std::error::Error>>(())
            })?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::seed_dev_data,
            commands::app_health,
            commands::login_online,
            commands::unlock_offline,
            commands::logout,
            commands::list_stations,
            commands::start_session,
            commands::stop_session,
            commands::resume_session,
            commands::live_charge,
            commands::open_pos_order,
            commands::get_order,
            commands::add_order_item,
            commands::void_order_item,
            commands::take_cash,
            commands::reverse_payment,
            commands::list_products,
            commands::list_sales,
            commands::adjust_inventory,
            commands::sales_report,
            commands::sales_today,
            commands::void_order,
            commands::list_backups,
            commands::backup_now,
            commands::restore_backup,
            commands::sync_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running playstation cafe");
}
