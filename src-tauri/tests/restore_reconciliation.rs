// Restoring a backup is the one path that swaps the database file underneath
// SQLite. Physical UAT restored a backup taken minutes earlier and the till came
// up with a corrupt index, then refused every sale with "database disk image is
// malformed"; once that was fixed the till would still have been unable to push,
// because a restored counter can sit behind what the cloud already accepted.
use playstation_cafe_lib::database;
use playstation_cafe_lib::dev;
use playstation_cafe_lib::domain::{inventory, orders, payments};
use playstation_cafe_lib::sync::engine;

async fn walk_in_with_one_drink(pool: &sqlx::SqlitePool, branch: &str, device: &str) -> String {
    let order = orders::open_pos_order(pool, branch, device, "u-c1").await.unwrap();
    let order_id = order["order_id"].as_str().unwrap().to_string();
    inventory::add_product_to_order(pool, branch, device, &order_id, "p-coke", 1, "u-c1")
        .await
        .unwrap();
    order_id
}

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("psc-{name}-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn sidecar(path: &std::path::Path, suffix: &str) -> std::path::PathBuf {
    let mut name = path.file_name().unwrap().to_os_string();
    name.push(suffix);
    path.with_file_name(name)
}

#[tokio::test]
async fn restore_does_not_let_a_stale_wal_corrupt_the_restored_database() {
    let dir = temp_dir("restore-wal");
    let busy = dir.join("busy.sqlite");
    let live = dir.join("branch.sqlite");

    // A till that has written a lot and not checkpointed: most of its content is
    // in the -wal, not in the main file.
    let pool = database::open_pool(&busy).await.unwrap();
    dev::seed_two_branches(&pool).await.unwrap();
    for i in 0..600 {
        sqlx::query(
            "INSERT INTO audit_logs (id, branch_id, user_id, action, entity_type, entity_id, created_at)
             VALUES (?, 'b1', 'u-c1', 'uat.filler', 'test', ?, ?)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(i.to_string())
        .bind(chrono::Utc::now().to_rfc3339())
        .execute(&pool)
        .await
        .unwrap();
    }

    // Copying the whole set while it is open is exactly the state a killed till
    // leaves on disk, and it keeps no open handle on the copy.
    for (from, to) in [
        (busy.clone(), live.clone()),
        (sidecar(&busy, "-wal"), sidecar(&live, "-wal")),
        (sidecar(&busy, "-shm"), sidecar(&live, "-shm")),
    ] {
        std::fs::copy(&from, &to).unwrap();
    }
    assert!(
        std::fs::metadata(sidecar(&live, "-wal")).unwrap().len() > 0,
        "the fixture must leave a live WAL behind, otherwise it proves nothing"
    );

    // A backup of a different, emptier database, taken the way backup_now takes it.
    let backup = dir.join("backup.sqlite");
    let other = dir.join("other.sqlite");
    let seed = database::open_pool(&other).await.unwrap();
    sqlx::query(&format!(
        "VACUUM INTO '{}'",
        backup.to_string_lossy().replace('\\', "/")
    ))
    .execute(&seed)
    .await
    .unwrap();
    seed.close().await;
    pool.close().await;

    std::fs::copy(&backup, live.with_extension("sqlite.restore")).unwrap();
    let restored = database::open_pool(&live).await.unwrap();

    let check: String = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_one(&restored)
        .await
        .unwrap();
    assert_eq!(check, "ok", "the restored database must not be corrupt");

    let stale_rows: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM audit_logs WHERE action = 'uat.filler'")
            .fetch_one(&restored)
            .await
            .unwrap();
    assert_eq!(
        stale_rows, 0,
        "the displaced database's WAL must not bleed into the restored one"
    );

    // The displaced copy keeps its own sidecars, so it stays recoverable.
    let pre = live.with_extension("sqlite.pre-restore");
    assert!(pre.exists(), "the pre-restore copy must be kept");
    assert!(
        sidecar(&pre, "-wal").exists(),
        "the WAL must follow the database it describes"
    );

    restored.close().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn reconciliation_moves_a_rewound_sequence_past_the_cloud() {
    let pool = database::open_memory().await.unwrap();
    sqlx::query("INSERT INTO device_sequence (device_id, next_sequence) VALUES ('d1', 27)")
        .execute(&pool)
        .await
        .unwrap();

    let snapshot = serde_json::json!({
        "sync_receipts": [
            { "event_id": "e-27", "device_id": "d1", "local_sequence": 27 },
            { "event_id": "e-30", "device_id": "d1", "local_sequence": 30 },
            { "event_id": "e-99", "device_id": "d-other", "local_sequence": 99 }
        ]
    });
    engine::apply_pull_snapshot(&pool, &snapshot).await.unwrap();

    let next: i64 =
        sqlx::query_scalar("SELECT next_sequence FROM device_sequence WHERE device_id = 'd1'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        next, 31,
        "the counter must clear every sequence the cloud already accepted"
    );
}

#[tokio::test]
async fn a_sale_after_a_restore_skips_receipts_the_cloud_already_holds() {
    let pool = database::open_memory().await.unwrap();
    dev::seed_two_branches(&pool).await.unwrap();

    // A restored backup that has lost the two orders the cloud closed today. The
    // pull carries their receipts, which is all the till needs to stay clear of
    // them: re-issuing one gets order.paid refused with a 409 for good, leaving
    // the sale paid on the till and unpaid in the cloud.
    let prefix = format!("B1-{}-", chrono::Utc::now().format("%Y%m%d"));
    let snapshot = serde_json::json!({
        "orders": [
            { "branch_id": "b1", "receipt_number": format!("{prefix}0001") },
            { "branch_id": "b1", "receipt_number": format!("{prefix}0002") },
            { "branch_id": "b1", "receipt_number": serde_json::Value::Null },
            { "branch_id": "b2", "receipt_number": format!("B2-{}-0009", chrono::Utc::now().format("%Y%m%d")) }
        ]
    });
    engine::apply_pull_snapshot(&pool, &snapshot).await.unwrap();

    let order = walk_in_with_one_drink(&pool, "b1", "d1").await;
    payments::take_cash(&pool, "b1", "d1", &order, 20_000, "u-c1")
        .await
        .expect("a restored till must still be able to close a sale");

    let receipt: String = sqlx::query_scalar("SELECT receipt_number FROM orders WHERE id = ?")
        .bind(&order)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        receipt,
        format!("{prefix}0003"),
        "the counter must clear the receipts the cloud already issued"
    );
}

#[tokio::test]
async fn recording_issued_receipts_only_ever_moves_the_mark_up() {
    let pool = database::open_memory().await.unwrap();
    let day = chrono::Utc::now().format("%Y%m%d").to_string();
    let high = serde_json::json!({
        "orders": [{ "branch_id": "b1", "receipt_number": format!("B1-{day}-0007") }]
    });
    let low = serde_json::json!({
        "orders": [{ "branch_id": "b1", "receipt_number": format!("B1-{day}-0002") }]
    });
    engine::apply_pull_snapshot(&pool, &high).await.unwrap();
    engine::apply_pull_snapshot(&pool, &low).await.unwrap();

    let mark: i64 = sqlx::query_scalar("SELECT last_sequence FROM receipt_high_water")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(mark, 7, "a later pull must not lower the mark");
}

#[tokio::test]
async fn reconciliation_never_rewinds_a_healthy_sequence() {
    let pool = database::open_memory().await.unwrap();
    sqlx::query("INSERT INTO device_sequence (device_id, next_sequence) VALUES ('d1', 40)")
        .execute(&pool)
        .await
        .unwrap();

    let snapshot = serde_json::json!({
        "sync_receipts": [{ "event_id": "e-30", "device_id": "d1", "local_sequence": 30 }]
    });
    engine::apply_pull_snapshot(&pool, &snapshot).await.unwrap();

    let next: i64 =
        sqlx::query_scalar("SELECT next_sequence FROM device_sequence WHERE device_id = 'd1'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(next, 40, "an ordinary pull must not touch the counter");
}
