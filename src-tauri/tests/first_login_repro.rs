use playstation_cafe_lib::database;

/// Captures the clean-install first-login failure before reference bootstrap exists.
/// A migrated empty SQLite has no hosted `user_branch_roles`, so the old
/// `login_online` local query cannot resolve a branch.
#[tokio::test]
async fn clean_sqlite_has_no_branch_assignment() {
    let pool = database::open_memory().await.expect("migrate");
    let roles: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM user_branch_roles")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(roles, 0, "clean install must not ship branch assignments");

    let branch: Option<String> = sqlx::query_scalar(
        "SELECT branch_id FROM user_branch_roles WHERE user_id = ? AND is_active = 1 LIMIT 1",
    )
    .bind("a11e0001-0a11-4000-a000-000000000002")
    .fetch_optional(&pool)
    .await
    .unwrap();

    assert!(
        branch.is_none(),
        "clean SQLite cannot resolve a hosted cashier branch"
    );
}
