use playstation_cafe_lib::database;
use playstation_cafe_lib::dev;
use playstation_cafe_lib::domain::gaming;

#[tokio::test]
async fn sqlite_rejects_stepped_and_ignores_reserved_increment() {
    let pool = database::open_memory().await.expect("migrate");
    dev::seed_two_branches(&pool).await.expect("seed");

    let stepped = sqlx::query(
        "INSERT INTO pricing_rules (
            id, branch_id, name, rule_type, rate_minor_per_hour, effective_from
         ) VALUES ('pr-step','b1','Stepped','stepped',3000,'2026-01-01T00:00:00Z')",
    )
    .execute(&pool)
    .await;
    assert!(stepped.is_err(), "SQLite must reject active stepped rules");

    let negative = sqlx::query(
        "INSERT INTO pricing_rules (
            id, branch_id, name, rule_type, rate_minor_per_hour, effective_from
         ) VALUES ('pr-neg','b1','Neg','linear',-1,'2026-01-01T00:00:00Z')",
    )
    .execute(&pool)
    .await;
    assert!(negative.is_err(), "SQLite must reject negative rates");

    sqlx::query("UPDATE pricing_rules SET billing_increment_seconds = 60 WHERE id = 'pr-b1'")
        .execute(&pool)
        .await
        .expect("reserved increment column may be stored");

    let start = gaming::start_session(&pool, "b1", "d1", "s-ps1", "u-c1")
        .await
        .expect("start");
    let snap = start["pricing_snapshot"].clone();
    assert_eq!(snap["rule_type"], "linear");
    assert_eq!(snap["rate_minor_per_hour"], 3000);
    assert!(
        snap["billing_increment_seconds"].is_null(),
        "activation must not copy increment into the live snapshot"
    );

    sqlx::query("UPDATE pricing_rules SET rate_minor_per_hour = 9999 WHERE id = 'pr-b1'")
        .execute(&pool)
        .await
        .unwrap();

    let session_id = start["session_id"].as_str().unwrap().to_string();
    let stored: String =
        sqlx::query_scalar("SELECT pricing_snapshot FROM gaming_sessions WHERE id = ?")
            .bind(&session_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let stored_snap: serde_json::Value = serde_json::from_str(&stored).unwrap();
    assert_eq!(stored_snap["rate_minor_per_hour"], 3000);

    let live = gaming::live_charge(&pool, &session_id).await.expect("live");
    let duration = live["duration_seconds"].as_i64().unwrap();
    let charge = live["charge_minor"].as_i64().unwrap();
    assert_eq!(charge, 3000 * duration / 3600);
}

#[tokio::test]
async fn sqlite_rejects_stepped_snapshot_at_charge_time() {
    let pool = database::open_memory().await.expect("migrate");
    dev::seed_two_branches(&pool).await.expect("seed");
    let start = gaming::start_session(&pool, "b1", "d1", "s-ps1", "u-c1")
        .await
        .expect("start");
    let session_id = start["session_id"].as_str().unwrap();
    sqlx::query(
        "UPDATE gaming_sessions SET pricing_snapshot = ? WHERE id = ?",
    )
    .bind(r#"{"rule_type":"stepped","rate_minor_per_hour":3000,"base_duration_seconds":3600,"base_charge_minor":3000,"step_duration_seconds":1800,"step_charge_minor":1500,"round_partial_step_up":true}"#)
    .bind(session_id)
    .execute(&pool)
    .await
    .unwrap();
    let live = gaming::live_charge(&pool, session_id).await;
    assert!(live.is_err(), "imported stepped snapshot must not charge");
}
