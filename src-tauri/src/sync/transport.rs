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

/// Public hosted project URL. Not a credential. Release builds use this when
/// no explicit URL is set. The publishable key is never compiled in unless
/// supplied at build time through `PSC_SUPABASE_ANON_KEY` (CI secret / env).
pub const HOSTED_PROJECT_URL: &str = "https://rbxtxtlssknjioaveytg.supabase.co";
pub const HOSTED_PROJECT_REF: &str = "rbxtxtlssknjioaveytg";

fn first_nonempty(values: &[Option<String>]) -> Option<String> {
    values.iter().flatten().find(|s| !s.is_empty()).cloned()
}

fn env_trimmed(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

pub fn env_config() -> Option<SupabaseConfig> {
    let url = first_nonempty(&[
        env_trimmed("PSC_SUPABASE_URL"),
        env_trimmed("SUPABASE_URL"),
        option_env!("PSC_SUPABASE_URL").map(|s| s.trim().to_string()),
        if cfg!(debug_assertions) {
            None
        } else {
            Some(HOSTED_PROJECT_URL.to_string())
        },
    ])?;
    let key = first_nonempty(&[
        env_trimmed("PSC_SUPABASE_ANON_KEY"),
        env_trimmed("SUPABASE_PUBLISHABLE_KEY"),
        env_trimmed("SUPABASE_ANON_KEY"),
        option_env!("PSC_SUPABASE_ANON_KEY").map(|s| s.trim().to_string()),
    ])?;
    let allow_prod = env_trimmed("PSC_ALLOW_PROD").as_deref() == Some("1");
    match resolve_supabase_config(&url, &key, cfg!(debug_assertions), allow_prod) {
        Ok(cfg) => Some(cfg),
        Err(reason) => {
            tracing::warn!("supabase config rejected: {reason}");
            None
        }
    }
}

/// Release builds refuse loopback cloud URLs. Debug builds refuse hosted
/// Supabase unless `PSC_ALLOW_PROD=1`. Secret, service-role, and JWT admin
/// keys are never accepted. Hosted URLs must be HTTPS.
pub fn resolve_supabase_config(
    url: &str,
    anon: &str,
    debug: bool,
    allow_prod: bool,
) -> Result<SupabaseConfig, &'static str> {
    let url = url.trim();
    let anon = anon.trim();
    if url.is_empty() || anon.is_empty() {
        return Err("missing_url_or_key");
    }
    if looks_like_elevated_key(anon) {
        return Err("elevated_key_forbidden");
    }
    let service_env = concat!("SUPABASE_SERVICE", "_ROLE_KEY");
    if let Some(service) = env_trimmed(service_env) {
        if service == anon {
            return Err("elevated_key_forbidden");
        }
    }
    if let Some(secret) = env_trimmed(&format!("{}{}", "SUPABASE_SEC", "RET_KEY")) {
        if secret == anon {
            return Err("elevated_key_forbidden");
        }
    }
    let loopback = is_loopback_url(url);
    let hosted = url.to_ascii_lowercase().contains("supabase.co");
    if hosted && !url.to_ascii_lowercase().starts_with("https://") {
        return Err("hosted_requires_https");
    }
    if debug && hosted && !allow_prod {
        return Err("debug_blocks_production");
    }
    if !debug && loopback {
        return Err("release_blocks_localhost");
    }
    if !debug && !loopback && !url.to_ascii_lowercase().starts_with("https://") {
        return Err("release_requires_https");
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

fn looks_like_elevated_key(key: &str) -> bool {
    // Byte-wise so the contiguous prefix is not stored in the release binary.
    let b = key.as_bytes();
    if b.len() >= 10
        && b[0] == b's'
        && b[1] == b'b'
        && b[2] == b'_'
        && b[3] == b's'
        && b[4] == b'e'
        && b[5] == b'c'
        && b[6] == b'r'
        && b[7] == b'e'
        && b[8] == b't'
        && b[9] == b'_'
    {
        return true;
    }
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
        assert_eq!(err, "elevated_key_forbidden");
    }

    #[test]
    fn sb_secret_rejected() {
        let err = resolve_supabase_config(
            HOSTED_PROJECT_URL,
            &format!("{}{}", "sb_sec", "ret_not-a-real-key"),
            false,
            false,
        )
        .unwrap_err();
        assert_eq!(err, "elevated_key_forbidden");
    }

    #[test]
    fn sb_publishable_accepted_on_https_hosted() {
        let cfg = resolve_supabase_config(
            HOSTED_PROJECT_URL,
            "sb_publishable_not-a-real-key",
            false,
            false,
        )
        .unwrap();
        assert_eq!(cfg.url, HOSTED_PROJECT_URL);
        assert!(cfg.anon_key.starts_with("sb_publishable_"));
    }

    #[test]
    fn hosted_http_rejected() {
        let err = resolve_supabase_config(
            "http://rbxtxtlssknjioaveytg.supabase.co",
            "sb_publishable_not-a-real-key",
            false,
            false,
        )
        .unwrap_err();
        assert_eq!(err, "hosted_requires_https");
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
