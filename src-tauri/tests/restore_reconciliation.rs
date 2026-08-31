// Restoring a backup is the one path that swaps the database file underneath
// SQLite. Physical UAT restored a backup taken minutes earlier and the till came
// up with a corrupt index, then refused every sale with "database disk image is
// malformed"; once that was fixed the till would still have been unable to push,
// because a restored counter can sit behind what the cloud already accepted.
use playstation_cafe_lib::database;
use playstation_cafe_lib::dev;
use playstation_cafe_lib::sync::engine;

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
            "INSERT INTO audit_logs (id, branch_id, actor_user_id, action, entity_type, entity_id, created_at)
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
