use tauri::State;

use crate::auth::{pin, session::Session, supabase_auth};
use crate::domain::{gaming, inventory, orders, payments};
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::sync::{engine as sync_engine, transport};

fn actor(state: &AppState) -> AppResult<Session> {
    state.sessions.require().map_err(AppError::Auth)
}

fn wake(state: &AppState) {
    state.sync.notify();
}

#[tauri::command]
pub async fn seed_dev_data(state: State<'_, AppState>) -> AppResult<()> {
    crate::dev::seed_two_branches(&state.db).await
}

#[tauri::command]
pub async fn app_health(state: State<'_, AppState>) -> AppResult<serde_json::Value> {
    let sync = sync_engine::status(&state.db).await?;
    Ok(serde_json::json!({
        "ok": true,
        "device_id": state.device_id,
        "session": state.sessions.get().map(|s| serde_json::json!({
            "user_id": s.user_id,
            "display_name": s.display_name,
            "branch_id": s.branch_id,
            "role": s.role,
            "offline": s.offline
        })),
        "sync": sync
    }))
}

#[tauri::command]
pub async fn login_online(
    state: State<'_, AppState>,
    email: String,
    password: String,
    pin: String,
) -> AppResult<serde_json::Value> {
    let cfg =
        transport::env_config().ok_or_else(|| AppError::Auth("cloud not configured".into()))?;
    let tokens = supabase_auth::password_login(&cfg, &email, &password).await?;
    let profiles =
        supabase_auth::fetch_profile(&cfg, &tokens.access_token, &tokens.user.id).await?;
    let profile = profiles
        .as_array()
        .and_then(|a| a.first())
        .cloned()
        .ok_or_else(|| AppError::Auth("profile missing".into()))?;
    let display = profile
        .get("display_name")
        .and_then(|v| v.as_str())
        .unwrap_or("User")
        .to_string();
    let admin = profile
        .get("is_system_admin")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    // Branch assignment is cached locally after first successful pairing/pull.
    let branch = sqlx::query_scalar::<_, String>(
        "SELECT branch_id FROM user_branch_roles WHERE user_id = ? AND is_active = 1 LIMIT 1",
    )
    .bind(&tokens.user.id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| {
        AppError::Auth("no local branch assignment; download reference data online first".into())
    })?;
    let role = sqlx::query_scalar::<_, String>(
        "SELECT role FROM user_branch_roles WHERE user_id = ? AND branch_id = ?",
    )
    .bind(&tokens.user.id)
    .bind(&branch)
    .fetch_one(&state.db)
    .await?;
    let hash = pin::hash_pin(&pin)?;
    pin::cache_offline_access(&state.db, &tokens.user.id, &display, &branch, &role, &hash).await?;
    state.sessions.set(Session {
        user_id: tokens.user.id.clone(),
        display_name: display.clone(),
        branch_id: branch.clone(),
        role: role.clone(),
        is_system_admin: admin,
        access_token: Some(tokens.access_token),
        refresh_token: Some(tokens.refresh_token),
        offline: false,
    });
    state.sync.notify();
    Ok(
        serde_json::json!({ "user_id": tokens.user.id, "display_name": display, "branch_id": branch, "role": role }),
    )
}

#[tauri::command]
pub async fn unlock_offline(
    state: State<'_, AppState>,
    user_id: String,
    pin: String,
) -> AppResult<serde_json::Value> {
    let (name, branch, role) = pin::unlock_offline(&state.db, &user_id, &pin).await?;
    state.sessions.set(Session {
        user_id: user_id.clone(),
        display_name: name.clone(),
        branch_id: branch.clone(),
        role: role.clone(),
        is_system_admin: false,
        access_token: None,
        refresh_token: None,
        offline: true,
    });
    Ok(
        serde_json::json!({ "user_id": user_id, "display_name": name, "branch_id": branch, "role": role, "offline": true }),
    )
}

#[tauri::command]
pub fn logout(state: State<'_, AppState>) -> AppResult<()> {
    state.sessions.clear();
    Ok(())
}

#[tauri::command]
pub async fn list_stations(state: State<'_, AppState>) -> AppResult<Vec<gaming::StationRow>> {
    let s = actor(&state)?;
    gaming::list_stations(&state.db, &s.branch_id).await
}

#[tauri::command]
pub async fn start_session(
    state: State<'_, AppState>,
    station_id: String,
) -> AppResult<serde_json::Value> {
    let s = actor(&state)?;
    let result = gaming::start_session(
        &state.db,
        &s.branch_id,
        &state.device_id,
        &station_id,
        &s.user_id,
    )
    .await?;
    wake(&state);
    Ok(result)
}

#[tauri::command]
pub async fn stop_session(
    state: State<'_, AppState>,
    session_id: String,
) -> AppResult<serde_json::Value> {
    let s = actor(&state)?;
    let result = gaming::stop_session(
        &state.db,
        &s.branch_id,
        &state.device_id,
        &session_id,
        &s.user_id,
    )
    .await?;
    wake(&state);
    Ok(result)
}

#[tauri::command]
pub async fn resume_session(
    state: State<'_, AppState>,
    session_id: String,
    reason: String,
) -> AppResult<serde_json::Value> {
    let s = actor(&state)?;
    let result = gaming::resume_session(
        &state.db,
        &s.branch_id,
        &state.device_id,
        &session_id,
        &s.user_id,
        &reason,
    )
    .await?;
    wake(&state);
    Ok(result)
}

#[tauri::command]
pub async fn live_charge(
    state: State<'_, AppState>,
    session_id: String,
) -> AppResult<serde_json::Value> {
    actor(&state)?;
    gaming::live_charge(&state.db, &session_id).await
}

#[tauri::command]
pub async fn open_pos_order(state: State<'_, AppState>) -> AppResult<serde_json::Value> {
    let s = actor(&state)?;
    let result = orders::open_pos_order(&state.db, &s.branch_id, &state.device_id, &s.user_id).await?;
    wake(&state);
    Ok(result)
}

#[tauri::command]
pub async fn get_order(
    state: State<'_, AppState>,
    order_id: String,
) -> AppResult<serde_json::Value> {
    let s = actor(&state)?;
    orders::get_order(&state.db, &s.branch_id, &order_id).await
}

#[tauri::command]
pub async fn add_order_item(
    state: State<'_, AppState>,
    order_id: String,
    product_id: String,
    quantity: i64,
) -> AppResult<serde_json::Value> {
    let s = actor(&state)?;
    let result = inventory::add_product_to_order(
        &state.db,
        &s.branch_id,
        &state.device_id,
        &order_id,
        &product_id,
        quantity,
        &s.user_id,
    )
    .await?;
    wake(&state);
    Ok(result)
}

#[tauri::command]
pub async fn void_order_item(
    state: State<'_, AppState>,
    item_id: String,
    reason: String,
) -> AppResult<serde_json::Value> {
    let s = actor(&state)?;
    let result = inventory::void_order_item(
        &state.db,
        &s.branch_id,
        &state.device_id,
        &item_id,
        &s.user_id,
        &reason,
    )
    .await?;
    wake(&state);
    Ok(result)
}

#[tauri::command]
pub async fn take_cash(
    state: State<'_, AppState>,
    order_id: String,
    tendered_minor: i64,
) -> AppResult<serde_json::Value> {
    let s = actor(&state)?;
    let result = payments::take_cash(
        &state.db,
        &s.branch_id,
        &state.device_id,
        &order_id,
        tendered_minor,
        &s.user_id,
    )
    .await?;
    wake(&state);
    Ok(result)
}

#[tauri::command]
pub async fn reverse_payment(
    state: State<'_, AppState>,
    order_id: String,
    reason: String,
) -> AppResult<serde_json::Value> {
    let s = actor(&state)?;
    if s.role != "admin" && !s.is_system_admin {
        return Err(AppError::Forbidden("reverse_payment requires admin".into()));
    }
    let result = payments::reverse_payment(
        &state.db,
        &s.branch_id,
        &state.device_id,
        &order_id,
        &s.user_id,
        &reason,
    )
    .await?;
    wake(&state);
    Ok(result)
}

#[tauri::command]
pub async fn adjust_inventory(
    state: State<'_, AppState>,
    product_id: String,
    movement_type: String,
    quantity_delta: i64,
    reason: String,
) -> AppResult<serde_json::Value> {
    let s = actor(&state)?;
    if s.role != "admin" && !s.is_system_admin {
        return Err(AppError::Forbidden(
            "inventory adjustment requires admin".into(),
        ));
    }
    let result = inventory::adjust(
        &state.db,
        &s.branch_id,
        &state.device_id,
        &product_id,
        &movement_type,
        quantity_delta,
        &reason,
        &s.user_id,
    )
    .await?;
    wake(&state);
    Ok(result)
}

#[tauri::command]
pub async fn list_products(state: State<'_, AppState>) -> AppResult<serde_json::Value> {
    let s = actor(&state)?;
    let rows: Vec<(String, String, Option<String>, i64, i64)> = sqlx::query_as(
        "SELECT p.id, p.name, p.name_ar,
                COALESCE(bp.sell_price_override_minor, p.default_sell_price_minor),
                COALESCE(b.quantity_on_hand, 0)
         FROM products p
         JOIN branch_products bp ON bp.product_id = p.id AND bp.branch_id = ?
         LEFT JOIN inventory_balances b ON b.product_id = p.id AND b.branch_id = ?
         WHERE p.is_active = 1 AND bp.is_active = 1
         ORDER BY p.name",
    )
    .bind(&s.branch_id)
    .bind(&s.branch_id)
    .fetch_all(&state.db)
    .await?;
    Ok(serde_json::json!(rows
        .into_iter()
        .map(|r| serde_json::json!({
            "id": r.0,
            "name": r.1,
            "name_ar": r.2,
            "sell_price_minor": r.3,
            "quantity_on_hand": r.4
        }))
        .collect::<Vec<_>>()))
}

#[tauri::command]
pub async fn list_sales(state: State<'_, AppState>) -> AppResult<serde_json::Value> {
    let s = actor(&state)?;
    let rows: Vec<(String, String, i64, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT id, status, total_minor, receipt_number, closed_at
         FROM orders
         WHERE branch_id = ? AND status IN ('paid','checkout_pending','open')
         ORDER BY opened_at DESC
         LIMIT 100",
    )
    .bind(&s.branch_id)
    .fetch_all(&state.db)
    .await?;
    Ok(serde_json::json!(rows
        .into_iter()
        .map(|r| serde_json::json!({
            "id": r.0,
            "status": r.1,
            "total_minor": r.2,
            "receipt_number": r.3,
            "closed_at": r.4
        }))
        .collect::<Vec<_>>()))
}

#[tauri::command]
pub async fn sales_report(
    state: State<'_, AppState>,
    from_utc: String,
    to_utc: String,
) -> AppResult<serde_json::Value> {
    let s = actor(&state)?;
    crate::reports::sales_summary(&state.db, Some(&s.branch_id), &from_utc, &to_utc).await
}

#[tauri::command]
pub async fn sales_today(state: State<'_, AppState>) -> AppResult<serde_json::Value> {
    let s = actor(&state)?;
    let (from_utc, to_utc) = crate::reports::cairo_today_utc_bounds();
    crate::reports::sales_summary(&state.db, Some(&s.branch_id), &from_utc, &to_utc).await
}

#[tauri::command]
pub async fn void_order(
    state: State<'_, AppState>,
    order_id: String,
    reason: String,
) -> AppResult<serde_json::Value> {
    let s = actor(&state)?;
    let result = orders::void_open_order(
        &state.db,
        &s.branch_id,
        &state.device_id,
        &order_id,
        &s.user_id,
        &reason,
    )
    .await?;
    wake(&state);
    Ok(result)
}

#[tauri::command]
pub async fn list_backups(state: State<'_, AppState>) -> AppResult<serde_json::Value> {
    let s = actor(&state)?;
    if s.role != "admin" && !s.is_system_admin {
        return Err(AppError::Forbidden("backup list requires admin".into()));
    }
    crate::backup::list_backups(&state.app_data_dir.join("backups"))
}

#[tauri::command]
pub async fn backup_now(state: State<'_, AppState>) -> AppResult<serde_json::Value> {
    let s = actor(&state)?;
    if s.role != "admin" && !s.is_system_admin {
        return Err(AppError::Forbidden("backup requires admin".into()));
    }
    crate::backup::backup_now(&state.db, &state.app_data_dir.join("backups")).await
}

#[tauri::command]
pub async fn restore_backup(
    state: State<'_, AppState>,
    backup_path: String,
) -> AppResult<serde_json::Value> {
    let s = actor(&state)?;
    if s.role != "admin" && !s.is_system_admin {
        return Err(AppError::Forbidden("restore requires admin".into()));
    }
    crate::backup::stage_restore(
        &std::path::PathBuf::from(backup_path),
        &state.app_data_dir.join("branch.sqlite"),
    )
    .await
}

#[tauri::command]
pub async fn sync_status(state: State<'_, AppState>) -> AppResult<serde_json::Value> {
    sync_engine::status(&state.db).await
}
