use playstation_cafe_lib::auth::pin;
use playstation_cafe_lib::auth::reference::{
    apply_reference_and_resolve, empty_snapshot, fetch_reference_snapshot, fixture_cashier_b1,
    fixture_cashier_b2, validate_snapshot,
};
use playstation_cafe_lib::database;
use playstation_cafe_lib::sync::sequence;
use playstation_cafe_lib::sync::transport::SupabaseConfig;
use sqlx::SqlitePool;

const B1: &str = "a11e0001-0a11-4000-b000-000000000001";
const B2: &str = "a11e0001-0a11-4000-b000-000000000002";
const C1: &str = "a11e0001-0a11-4000-a000-000000000002";

async fn next_seq(pool: &SqlitePool, device_id: &str) -> i64 {
    let mut tx = pool.begin().await.unwrap();
    let n = sequence::next_in_tx(&mut tx, device_id).await.unwrap();
    tx.commit().await.unwrap();
    n
}

#[tokio::test]
async fn clean_sqlite_cannot_resolve_branch_before_bootstrap() {
    let pool = database::open_memory().await.unwrap();
    let branch: Option<String> = sqlx::query_scalar(
        "SELECT branch_id FROM user_branch_roles WHERE user_id = ? AND is_active = 1 LIMIT 1",
    )
    .bind(C1)
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert!(
        branch.is_none(),
        "reproduced: empty SQLite has no hosted assignment"
    );
}

#[tokio::test]
async fn empty_db_accepts_authenticated_reference_snapshot() {
    let pool = database::open_memory().await.unwrap();
    let snap = fixture_cashier_b1();
    let assigned = apply_reference_and_resolve(&pool, &snap, "1357")
        .await
        .expect("bootstrap");
    assert_eq!(assigned.branch_id, B1);
    assert_eq!(assigned.role, "cashier");
    assert!(!assigned.is_system_admin);

    let stations: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM stations WHERE branch_id = ?")
            .bind(B1)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(stations >= 1);
    let pricing: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pricing_rules WHERE branch_id = ? AND rule_type = 'linear'",
    )
    .bind(B1)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(pricing, 1);
    let products: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM products")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(products >= 1);
    let inv: i64 = sqlx::query_scalar(
        "SELECT quantity_on_hand FROM inventory_balances WHERE branch_id = ? LIMIT 1",
    )
    .bind(B1)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(inv, 20);
    let methods: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM payment_methods")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(methods >= 1);

    let resolved: String = sqlx::query_scalar(
        "SELECT branch_id FROM user_branch_roles WHERE user_id = ? AND is_active = 1 LIMIT 1",
    )
    .bind(C1)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(resolved, B1);

    let pin_rows: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM offline_access_cache WHERE user_id = ?")
            .bind(C1)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(pin_rows, 1);
}

#[tokio::test]
async fn cashier_bootstrap_drops_foreign_branch_rows() {
    let pool = database::open_memory().await.unwrap();
    apply_reference_and_resolve(&pool, &fixture_cashier_b1(), "1357")
        .await
        .unwrap();
    let b2_branches: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM branches WHERE id = ?")
        .bind(B2)
        .fetch_one(&pool)
        .await
        .unwrap();
    let b2_stations: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM stations WHERE branch_id = ?")
            .bind(B2)
            .fetch_one(&pool)
            .await
            .unwrap();
    let b2_inv: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM inventory_balances WHERE branch_id = ?")
            .bind(B2)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(b2_branches, 0, "B1 cashier must not cache B2 branch");
    assert_eq!(b2_stations, 0);
    assert_eq!(b2_inv, 0);
}

#[tokio::test]
async fn b2_cashier_bootstrap_does_not_cache_b1() {
    let pool = database::open_memory().await.unwrap();
    let assigned = apply_reference_and_resolve(&pool, &fixture_cashier_b2(), "2468")
        .await
        .unwrap();
    assert_eq!(assigned.branch_id, B2);
    let b1: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM branches WHERE id = ?")
        .bind(B1)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(b1, 0);
}

#[tokio::test]
async fn inactive_assignment_is_rejected_and_writes_nothing() {
    let pool = database::open_memory().await.unwrap();
    let mut snap = fixture_cashier_b1();
    snap.roles[0]["is_active"] = serde_json::json!(false);
    let err = apply_reference_and_resolve(&pool, &snap, "1357")
        .await
        .unwrap_err();
    assert!(err.to_string().contains("no active branch assignment"));
    let roles: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM user_branch_roles")
        .fetch_one(&pool)
        .await
        .unwrap();
    let pins: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM offline_access_cache")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(roles, 0);
    assert_eq!(pins, 0);
}

#[tokio::test]
async fn two_cashier_assignments_rejected() {
    let mut snap = fixture_cashier_b1();
    snap.roles.push(serde_json::json!({
        "user_id": C1,
        "branch_id": B2,
        "role": "cashier",
        "is_active": true,
        "offline_access_allowed": true,
        "created_at": "2026-08-30T00:00:00Z"
    }));
    let err = validate_snapshot(&snap).unwrap_err();
    assert!(err.to_string().contains("exactly one active branch"));
}

#[tokio::test]
async fn network_failure_does_not_cache_pin() {
    let pool = database::open_memory().await.unwrap();
    let cfg = SupabaseConfig {
        url: "http://127.0.0.1:1".into(),
        anon_key: "sb_publishable_test".into(),
    };
    let err = fetch_reference_snapshot(&cfg, "not-a-jwt", C1)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("reference download failed"));
    assert!(!err.to_string().contains("eyJ"));
    let pins: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM offline_access_cache")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(pins, 0);
}

#[tokio::test]
async fn existing_offline_pin_survives_failed_online_bootstrap() {
    let pool = database::open_memory().await.unwrap();
    apply_reference_and_resolve(&pool, &fixture_cashier_b1(), "1357")
        .await
        .unwrap();
    let cfg = SupabaseConfig {
        url: "http://127.0.0.1:1".into(),
        anon_key: "sb_publishable_test".into(),
    };
    let _ = fetch_reference_snapshot(&cfg, "not-a-jwt", C1)
        .await
        .unwrap_err();
    let unlocked = pin::unlock_offline(&pool, C1, "1357").await.unwrap();
    assert_eq!(unlocked.1, B1);
}

#[tokio::test]
async fn failed_validate_does_not_create_offline_pin() {
    let pool = database::open_memory().await.unwrap();
    let err = apply_reference_and_resolve(&pool, &empty_snapshot(C1), "1357")
        .await
        .unwrap_err();
    assert!(err.to_string().contains("profile missing"));
    let pins: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM offline_access_cache")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(pins, 0);
}

#[tokio::test]
async fn local_first_sequence_is_one() {
    let pool = database::open_memory().await.unwrap();
    assert_eq!(next_seq(&pool, "local-writer").await, 1);
    assert_eq!(next_seq(&pool, "local-writer").await, 2);
}

#[tokio::test]
async fn bootstrap_does_not_advance_cloud_or_local_business_sequence() {
    let pool = database::open_memory().await.unwrap();
    apply_reference_and_resolve(&pool, &fixture_cashier_b1(), "1357")
        .await
        .unwrap();
    let outbox: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sync_outbox")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(outbox, 0);
    assert_eq!(next_seq(&pool, "a11e0001-0a11-4000-d000-000000000001").await, 1);
}
