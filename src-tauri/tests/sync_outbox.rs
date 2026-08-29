use playstation_cafe_lib::database;
use playstation_cafe_lib::sync::{engine, outbox};

async fn insert_outbox(pool: &sqlx::SqlitePool, event_id: &str, status: &str) {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO sync_outbox (
            event_id, sequence, event_type, aggregate_type, aggregate_id,
            branch_id, device_id, payload_json, payload_hash, created_at,
            attempt_count, next_attempt_at, sync_status
         ) VALUES (?, 1, 'order.opened', 'order', 'o1', 'b1', 'd1', '{}', 'hash', ?, 0, ?, ?)",
    )
    .bind(event_id)
    .bind(&now)
    .bind(&now)
    .bind(status)
    .execute(pool)
    .await
    .expect("insert outbox");
}

#[tokio::test]
async fn crash_mid_push_recovers_sending_as_pending() {
    let pool = database::open_memory().await.expect("migrate");
    insert_outbox(&pool, "evt-send", "sending").await;
    let recovered = outbox::recover_stale_sending(&pool).await.unwrap();
    assert_eq!(recovered, 1);
    let pending = outbox::pending(&pool, 20).await.unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].event_id, "evt-send");
    assert_eq!(pending[0].sync_status, "pending");
}

#[tokio::test]
async fn sending_then_already_processed_marks_synced() {
    let pool = database::open_memory().await.expect("migrate");
    insert_outbox(&pool, "evt-ok", "pending").await;
    assert!(outbox::mark_sending(&pool, "evt-ok").await.unwrap());
    outbox::mark_synced(&pool, "evt-ok").await.unwrap();
    let status: String = sqlx::query_scalar("SELECT sync_status FROM sync_outbox WHERE event_id = 'evt-ok'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "synced");
    assert!(outbox::pending(&pool, 20).await.unwrap().is_empty());
}

#[tokio::test]
async fn payload_mismatch_is_dead_lettered() {
    let pool = database::open_memory().await.expect("migrate");
    insert_outbox(&pool, "evt-dead", "sending").await;
    outbox::mark_dead(&pool, "evt-dead", "event_id_payload_mismatch").await.unwrap();
    assert!(outbox::pending(&pool, 20).await.unwrap().is_empty());
    let attempt: i64 =
        sqlx::query_scalar("SELECT attempt_count FROM sync_outbox WHERE event_id = 'evt-dead'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(attempt, 99);
}

#[tokio::test]
async fn pull_before_push_reconciles_matching_receipts() {
    let pool = database::open_memory().await.expect("migrate");
    insert_outbox(&pool, "evt-pulled", "pending").await;
    sqlx::query("UPDATE sync_state SET restore_reconciliation_required = 1 WHERE id = 1")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        engine::pull_cursor(&pool).await,
        "1970-01-01T00:00:00Z"
    );
    let marked = engine::apply_pull_snapshot(
        &pool,
        &serde_json::json!({
            "sync_receipts": [{ "event_id": "evt-pulled" }],
            "orders": []
        }),
    )
    .await
    .unwrap();
    assert_eq!(marked, 1);
    let status: String =
        sqlx::query_scalar("SELECT sync_status FROM sync_outbox WHERE event_id = 'evt-pulled'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "synced");
    let recon: i64 =
        sqlx::query_scalar("SELECT restore_reconciliation_required FROM sync_state WHERE id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(recon, 0);
    assert_ne!(engine::pull_cursor(&pool).await, "1970-01-01T00:00:00Z");
}
