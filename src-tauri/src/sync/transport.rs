use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone)]
pub struct SupabaseConfig {
    pub url: String,
    pub anon_key: String,
}

#[derive(Debug, Deserialize)]
pub struct ApplyResult {
    pub status: String,
    pub event_id: Option<String>,
    pub local_sequence: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct ApplyRequest {
    pub p_event_id: String,
    pub p_branch_id: String,
    pub p_device_id: String,
    pub p_local_sequence: i64,
    pub p_event_type: String,
    pub p_payload: serde_json::Value,
    pub p_payload_hash: String,
}

pub async fn apply_domain_event(
    cfg: &SupabaseConfig,
    access_token: &str,
    req: &ApplyRequest,
) -> AppResult<ApplyResult> {
    let client = reqwest::Client::new();
    let url = format!(
        "{}/rest/v1/rpc/apply_domain_event",
        cfg.url.trim_end_matches('/')
    );
    let res = client
        .post(url)
        .header(AUTHORIZATION, format!("Bearer {access_token}"))
        .header("apikey", &cfg.anon_key)
        .header(CONTENT_TYPE, "application/json")
        .json(req)
        .send()
        .await
        .map_err(|e| AppError::Sync(e.to_string()))?;

    let status = res.status();
    let body = res
        .text()
        .await
        .map_err(|e| AppError::Sync(e.to_string()))?;
    if !status.is_success() {
        if body.contains("sequence_gap") {
            return Err(AppError::Sync(format!("sequence_gap: {body}")));
        }
        return Err(AppError::Sync(format!("rpc {status}: {body}")));
    }
    serde_json::from_str(&body).map_err(|e| AppError::Sync(e.to_string()))
}

pub async fn pull_branch_since(
    cfg: &SupabaseConfig,
    access_token: &str,
    branch_id: &str,
    after: &str,
) -> AppResult<serde_json::Value> {
    let client = reqwest::Client::new();
    let url = format!(
        "{}/rest/v1/rpc/pull_branch_since",
        cfg.url.trim_end_matches('/')
    );
    let res = client
        .post(url)
        .header(AUTHORIZATION, format!("Bearer {access_token}"))
        .header("apikey", &cfg.anon_key)
        .header(CONTENT_TYPE, "application/json")
        .json(&json!({ "p_branch_id": branch_id, "p_after": after }))
        .send()
        .await
        .map_err(|e| AppError::Sync(e.to_string()))?;
    let status = res.status();
    let body = res
        .text()
        .await
        .map_err(|e| AppError::Sync(e.to_string()))?;
    if !status.is_success() {
        return Err(AppError::Sync(format!("pull {status}: {body}")));
    }
    Ok(serde_json::from_str(&body)?)
}

pub fn env_config() -> Option<SupabaseConfig> {
    let url = std::env::var("PSC_SUPABASE_URL").ok()?;
    let anon = std::env::var("PSC_SUPABASE_ANON_KEY").ok()?;
    if url.contains("supabase.co")
        && cfg!(debug_assertions)
        && std::env::var("PSC_ALLOW_PROD").ok().as_deref() != Some("1")
    {
        // Debug builds refuse hosted production unless explicitly overridden.
        if std::env::var("PSC_ENV").ok().as_deref() == Some("production") {
            return None;
        }
    }
    Some(SupabaseConfig {
        url,
        anon_key: anon,
    })
}
