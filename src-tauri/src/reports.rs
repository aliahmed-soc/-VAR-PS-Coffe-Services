use chrono::{FixedOffset, TimeZone, Utc};
use sqlx::SqlitePool;

use crate::error::AppResult;

/// Africa/Cairo is UTC+2 with no DST. Report day bounds are computed here, not in the UI.
pub fn cairo_today_utc_bounds() -> (String, String) {
    let cairo = FixedOffset::east_opt(2 * 3600).expect("cairo offset");
    let now = Utc::now().with_timezone(&cairo);
    let start_naive = now.date_naive().and_hms_opt(0, 0, 0).expect("midnight");
    let start = cairo.from_local_datetime(&start_naive).single().expect("cairo midnight");
    let end = start + chrono::Duration::days(1);
    (
        start.with_timezone(&Utc).to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        end.with_timezone(&Utc).to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
    )
}

pub async fn sales_summary(
    pool: &SqlitePool,
    branch_id: Option<&str>,
    from_utc: &str,
    to_utc: &str,
) -> AppResult<serde_json::Value> {
    let (gaming, product, count): (i64, i64, i64) = if let Some(branch) = branch_id {
        sqlx::query_as(
            "SELECT COALESCE(SUM(gaming_subtotal_minor),0),
                    COALESCE(SUM(product_subtotal_minor),0),
                    COUNT(*)
             FROM orders
             WHERE status = 'paid' AND branch_id = ? AND closed_at >= ? AND closed_at < ?",
        )
        .bind(branch)
        .bind(from_utc)
        .bind(to_utc)
        .fetch_one(pool)
        .await?
    } else {
        sqlx::query_as(
            "SELECT COALESCE(SUM(gaming_subtotal_minor),0),
                    COALESCE(SUM(product_subtotal_minor),0),
                    COUNT(*)
             FROM orders
             WHERE status = 'paid' AND closed_at >= ? AND closed_at < ?",
        )
        .bind(from_utc)
        .bind(to_utc)
        .fetch_one(pool)
        .await?
    };
    Ok(serde_json::json!({
        "gaming_revenue_minor": gaming,
        "product_revenue_minor": product,
        "sales_revenue_minor": gaming + product,
        "paid_orders": count,
        "from": from_utc,
        "to": to_utc,
        "branch_id": branch_id,
        "freshness": "local"
    }))
}
