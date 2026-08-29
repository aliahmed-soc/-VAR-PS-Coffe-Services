use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use chrono::{Duration, Utc};
use rand::rngs::OsRng;
use sqlx::SqlitePool;

use crate::error::{AppError, AppResult};

pub const OFFLINE_TTL_HOURS: i64 = 72;

pub fn offline_authorization_valid(
    expires_rfc3339: &str,
    now: chrono::DateTime<Utc>,
) -> Result<bool, String> {
    let exp = chrono::DateTime::parse_from_rfc3339(expires_rfc3339).map_err(|e| e.to_string())?;
    Ok(exp.with_timezone(&Utc) >= now)
}

pub fn hash_pin(pin: &str) -> AppResult<String> {
    if pin.len() < 4 {
        return Err(AppError::domain("PIN must be at least 4 characters"));
    }
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(pin.as_bytes(), &salt)
        .map_err(|e| AppError::Auth(e.to_string()))?
        .to_string();
    Ok(hash)
}

pub fn verify_pin(pin: &str, hash: &str) -> bool {
    PasswordHash::new(hash)
        .ok()
        .and_then(|parsed| {
            Argon2::default()
                .verify_password(pin.as_bytes(), &parsed)
                .ok()
        })
        .is_some()
}

pub async fn purge_expired(pool: &SqlitePool, now: chrono::DateTime<Utc>) -> AppResult<u64> {
    let result = sqlx::query(
        "DELETE FROM offline_access_cache WHERE authorization_expires_at < ?",
    )
    .bind(now.to_rfc3339())
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

pub async fn cache_offline_access(
    pool: &SqlitePool,
    user_id: &str,
    display_name: &str,
    branch_id: &str,
    role: &str,
    pin_hash: &str,
) -> AppResult<()> {
    let now = Utc::now();
    purge_expired(pool, now).await?;
    let expires = (now + Duration::hours(OFFLINE_TTL_HOURS)).to_rfc3339();
    sqlx::query(
        "INSERT INTO offline_access_cache (
            user_id, display_name, branch_id, role, pin_hash,
            authorization_expires_at, last_online_auth_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(user_id) DO UPDATE SET
            display_name = excluded.display_name,
            branch_id = excluded.branch_id,
            role = excluded.role,
            pin_hash = excluded.pin_hash,
            authorization_expires_at = excluded.authorization_expires_at,
            last_online_auth_at = excluded.last_online_auth_at",
    )
    .bind(user_id)
    .bind(display_name)
    .bind(branch_id)
    .bind(role)
    .bind(pin_hash)
    .bind(&expires)
    .bind(now.to_rfc3339())
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn unlock_offline(
    pool: &SqlitePool,
    user_id: &str,
    pin: &str,
) -> AppResult<(String, String, String, String)> {
    let row: Option<(String, String, String, String, String)> = sqlx::query_as(
        "SELECT display_name, branch_id, role, pin_hash, authorization_expires_at
         FROM offline_access_cache WHERE user_id = ?",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    let Some((name, branch, role, hash, expires)) = row else {
        return Err(AppError::Auth("no offline access on this device".into()));
    };
    if !offline_authorization_valid(&expires, Utc::now()).map_err(AppError::Auth)? {
        sqlx::query("DELETE FROM offline_access_cache WHERE user_id = ?")
            .bind(user_id)
            .execute(pool)
            .await?;
        return Err(AppError::Auth(
            "offline authorization expired; online login required".into(),
        ));
    }
    if !verify_pin(pin, &hash) {
        return Err(AppError::Auth("invalid PIN".into()));
    }
    Ok((name, branch, role, expires))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pin_roundtrip() {
        let hash = hash_pin("1357").unwrap();
        assert!(verify_pin("1357", &hash));
        assert!(!verify_pin("0000", &hash));
        assert!(!hash.contains("1357"));
    }

    #[test]
    fn offline_ttl_is_72_hours() {
        assert_eq!(OFFLINE_TTL_HOURS, 72);
        let issued = Utc::now();
        let expires = (issued + Duration::hours(OFFLINE_TTL_HOURS)).to_rfc3339();
        assert!(offline_authorization_valid(&expires, issued + Duration::hours(71)).unwrap());
        assert!(!offline_authorization_valid(&expires, issued + Duration::hours(73)).unwrap());
    }

    #[tokio::test]
    async fn expired_cache_cannot_unlock() {
        let pool = crate::database::open_memory().await.unwrap();
        let hash = hash_pin("1357").unwrap();
        let past = (Utc::now() - Duration::hours(1)).to_rfc3339();
        sqlx::query(
            "INSERT INTO offline_access_cache (
                user_id, display_name, branch_id, role, pin_hash,
                authorization_expires_at, last_online_auth_at
             ) VALUES ('u-exp','Exp','b1','cashier',?,?,?)",
        )
        .bind(&hash)
        .bind(&past)
        .bind(&past)
        .execute(&pool)
        .await
        .unwrap();
        let err = unlock_offline(&pool, "u-exp", "1357").await.unwrap_err();
        assert!(
            err.to_string().contains("expired"),
            "unexpected error: {err}"
        );
        let left: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM offline_access_cache WHERE user_id = 'u-exp'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(left, 0, "purge must drop expired rows");
    }
}
