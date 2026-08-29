use sqlx::SqliteConnection;

use crate::error::AppResult;

pub async fn next_in_tx(tx: &mut SqliteConnection, device_id: &str) -> AppResult<i64> {
    sqlx::query(
        "INSERT INTO device_sequence (device_id, next_sequence) VALUES (?, 1)
         ON CONFLICT(device_id) DO NOTHING",
    )
    .bind(device_id)
    .execute(&mut *tx)
    .await?;

    let current: i64 =
        sqlx::query_scalar("SELECT next_sequence FROM device_sequence WHERE device_id = ?")
            .bind(device_id)
            .fetch_one(&mut *tx)
            .await?;

    sqlx::query("UPDATE device_sequence SET next_sequence = next_sequence + 1 WHERE device_id = ?")
        .bind(device_id)
        .execute(&mut *tx)
        .await?;

    Ok(current)
}
