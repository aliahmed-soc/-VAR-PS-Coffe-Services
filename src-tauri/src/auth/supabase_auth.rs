use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde::Deserialize;

use crate::error::{AppError, AppResult};
use crate::sync::transport::SupabaseConfig;

#[derive(Debug, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub user: AuthUser,
}

#[derive(Debug, Deserialize)]
pub struct AuthUser {
    pub id: String,
    pub email: Option<String>,
}

pub async fn password_login(cfg: &SupabaseConfig, email: &str, password: &str) -> AppResult<TokenResponse> {
    let client = reqwest::Client::new();
    let url = format!("{}/auth/v1/token?grant_type=password", cfg.url.trim_end_matches('/'));
    let res = client
        .post(url)
        .header("apikey", &cfg.anon_key)
        .header(CONTENT_TYPE, "application/json")
        .json(&serde_json::json!({ "email": email, "password": password }))
        .send()
        .await
        .map_err(|e| AppError::Auth(e.to_string()))?;
    let status = res.status();
    let body = res.text().await.map_err(|e| AppError::Auth(e.to_string()))?;
    if !status.is_success() {
        return Err(AppError::Auth(format!("login failed: {body}")));
    }
    serde_json::from_str(&body).map_err(|e| AppError::Auth(e.to_string()))
}

pub async fn refresh(cfg: &SupabaseConfig, refresh_token: &str) -> AppResult<TokenResponse> {
    let client = reqwest::Client::new();
    let url = format!("{}/auth/v1/token?grant_type=refresh_token", cfg.url.trim_end_matches('/'));
    let res = client
        .post(url)
        .header("apikey", &cfg.anon_key)
        .header(CONTENT_TYPE, "application/json")
        .json(&serde_json::json!({ "refresh_token": refresh_token }))
        .send()
        .await
        .map_err(|e| AppError::Auth(e.to_string()))?;
    let status = res.status();
    let body = res.text().await.map_err(|e| AppError::Auth(e.to_string()))?;
    if !status.is_success() {
        return Err(AppError::Auth(format!("refresh failed: {body}")));
    }
    serde_json::from_str(&body).map_err(|e| AppError::Auth(e.to_string()))
}

pub async fn fetch_profile(
    cfg: &SupabaseConfig,
    access_token: &str,
    user_id: &str,
) -> AppResult<serde_json::Value> {
    let client = reqwest::Client::new();
    let url = format!(
        "{}/rest/v1/user_profiles?user_id=eq.{user_id}&select=*",
        cfg.url.trim_end_matches('/')
    );
    let res = client
        .get(url)
        .header(AUTHORIZATION, format!("Bearer {access_token}"))
        .header("apikey", &cfg.anon_key)
        .send()
        .await
        .map_err(|e| AppError::Auth(e.to_string()))?;
    let body = res.text().await.map_err(|e| AppError::Auth(e.to_string()))?;
    Ok(serde_json::from_str(&body)?)
}
