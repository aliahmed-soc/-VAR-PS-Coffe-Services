use std::time::Duration;

use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::error::{AppError, AppResult};

pub fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .connect_timeout(Duration::from_secs(8))
        .build()
        .expect("http client")
}

fn sanitize_rpc_error(kind: &str, status: reqwest::StatusCode, body: &str) -> AppError {
    if body.contains("sequence_gap") {
        return AppError::Sync("sequence_gap".into());
    }
    if body.contains("event_id_payload_mismatch") {
        return AppError::Sync(format!("{kind} {status}: event_id_payload_mismatch"));
    }
    if status.as_u16() == 401 || status.as_u16() == 403 {
        return AppError::Sync(format!("{kind} {status}: unauthorized"));
    }
    AppError::Sync(format!("{kind} {status}"))
}

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
    let client = http_client();
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
        return Err(sanitize_rpc_error("rpc", status, &body));
    }
    serde_json::from_str(&body).map_err(|e| AppError::Sync(e.to_string()))
}

pub async fn pull_branch_since(
    cfg: &SupabaseConfig,
    access_token: &str,
    branch_id: &str,
    after: &str,
) -> AppResult<serde_json::Value> {
    let client = http_client();
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
        return Err(sanitize_rpc_error("pull", status, &body));
    }
    Ok(serde_json::from_str(&body)?)
}

pub fn env_config() -> Option<SupabaseConfig> {
    let url = std::env::var("PSC_SUPABASE_URL").ok()?;
    let anon = std::env::var("PSC_SUPABASE_ANON_KEY").ok()?;
    let allow_prod = std::env::var("PSC_ALLOW_PROD").ok().as_deref() == Some("1");
    match resolve_supabase_config(&url, &anon, cfg!(debug_assertions), allow_prod) {
        Ok(cfg) => Some(cfg),
        Err(reason) => {
            tracing::warn!("supabase config rejected: {reason}");
            None
        }
    }
}

/// Release builds refuse loopback cloud URLs. Debug builds refuse hosted
/// Supabase unless `PSC_ALLOW_PROD=1`. Service-role keys are never accepted.
pub fn resolve_supabase_config(
    url: &str,
    anon: &str,
    debug: bool,
    allow_prod: bool,
) -> Result<SupabaseConfig, &'static str> {
    if url.is_empty() || anon.is_empty() {
        return Err("missing_url_or_key");
    }
    if looks_like_service_role(anon) {
        return Err("service_role_forbidden");
    }
    if let Ok(service) = std::env::var("SUPABASE_SERVICE_ROLE_KEY") {
        if !service.is_empty() && service == anon {
            return Err("service_role_forbidden");
        }
    }
    let loopback = is_loopback_url(url);
    let hosted = url.contains("supabase.co");
    if debug && hosted && !allow_prod {
        return Err("debug_blocks_production");
    }
    if !debug && loopback {
        return Err("release_blocks_localhost");
    }
    Ok(SupabaseConfig {
        url: url.to_string(),
        anon_key: anon.to_string(),
    })
}

fn is_loopback_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    lower.contains("localhost") || lower.contains("127.0.0.1") || lower.contains("[::1]")
}

fn looks_like_service_role(key: &str) -> bool {
    if key.contains("service_role") || key.contains("supabase_admin") {
        return true;
    }
    jwt_role(key).is_some_and(|role| role == "service_role" || role == "supabase_admin")
}

fn jwt_role(token: &str) -> Option<String> {
    let payload = token.split('.').nth(1)?;
    let bytes = b64url_decode(payload)?;
    let value: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    value.get("role")?.as_str().map(|s| s.to_string())
}

fn b64url_decode(data: &str) -> Option<Vec<u8>> {
    fn val(c: u8) -> Option<u8> {
        match c {
            b'A'..=b'Z' => Some(c - b'A'),
            b'a'..=b'z' => Some(c - b'a' + 26),
            b'0'..=b'9' => Some(c - b'0' + 52),
            b'+' | b'-' => Some(62),
            b'/' | b'_' => Some(63),
            _ => None,
        }
    }
    let mut out = Vec::new();
    let mut buf = 0u32;
    let mut bits = 0u32;
    for &c in data.as_bytes() {
        if c == b'=' {
            break;
        }
        let v = u32::from(val(c)?);
        buf = (buf << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_blocks_hosted_without_override() {
        let err = resolve_supabase_config(
            "https://abc.supabase.co",
            "anon-local-key",
            true,
            false,
        )
        .unwrap_err();
        assert_eq!(err, "debug_blocks_production");
    }

    #[test]
    fn release_blocks_localhost() {
        let err = resolve_supabase_config(
            "http://127.0.0.1:54321",
            "anon-local-key",
            false,
            false,
        )
        .unwrap_err();
        assert_eq!(err, "release_blocks_localhost");
    }

    #[test]
    fn service_role_jwt_rejected() {
        let payload = b64url_encode(br#"{"role":"service_role","iss":"supabase"}"#);
        let key = format!("eyJhbGciOiJub25lIn0.{payload}.sig");
        let err = resolve_supabase_config("https://abc.supabase.co", &key, false, false)
            .unwrap_err();
        assert_eq!(err, "service_role_forbidden");
    }

    #[test]
    fn release_accepts_hosted_anon() {
        let cfg = resolve_supabase_config(
            "https://abc.supabase.co",
            "anon-local-key",
            false,
            false,
        )
        .unwrap();
        assert_eq!(cfg.url, "https://abc.supabase.co");
    }

    fn b64url_encode(bytes: &[u8]) -> String {
        const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
        let mut out = String::new();
        let mut i = 0;
        while i < bytes.len() {
            let b0 = bytes[i];
            let b1 = if i + 1 < bytes.len() { bytes[i + 1] } else { 0 };
            let b2 = if i + 2 < bytes.len() { bytes[i + 2] } else { 0 };
            out.push(T[(b0 >> 2) as usize] as char);
            out.push(T[(((b0 & 3) << 4) | (b1 >> 4)) as usize] as char);
            if i + 1 < bytes.len() {
                out.push(T[(((b1 & 15) << 2) | (b2 >> 6)) as usize] as char);
            }
            if i + 2 < bytes.len() {
                out.push(T[(b2 & 63) as usize] as char);
            }
            i += 3;
        }
        out
    }
}
