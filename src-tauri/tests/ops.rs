use playstation_cafe_lib::backup;
use playstation_cafe_lib::database;
use playstation_cafe_lib::dev;
use playstation_cafe_lib::domain::{gaming, inventory, payments};
use playstation_cafe_lib::reports;

#[tokio::test]
async fn reports_reconcile_paid_identity_and_restore_requires_pull() {
    let pool = database::open_memory().await.expect("migrate");
    dev::seed_two_branches(&pool).await.expect("seed");
    let start = gaming::start_session(&pool, "b1", "d1", "s-ps1", "u-c1")
        .await
        .unwrap();
    let session_id = start["session_id"].as_str().unwrap().to_string();
    let order_id = start["order_id"].as_str().unwrap().to_string();
    inventory::add_product_to_order(&pool, "b1", "d1", &order_id, "p-coke", 1, "u-c1")
        .await
        .unwrap();
    gaming::stop_session(&pool, "b1", "d1", &session_id, "u-c1")
        .await
        .unwrap();
    payments::take_cash(&pool, "b1", "d1", &order_id, 20_000, "u-c1")
        .await
        .unwrap();

    let report = reports::sales_summary(&pool, Some("b1"), "1970-01-01T00:00:00Z", "2099-01-01T00:00:00Z")
        .await
        .unwrap();
    let gaming = report["gaming_revenue_minor"].as_i64().unwrap();
    let product = report["product_revenue_minor"].as_i64().unwrap();
    let tax = report["tax_minor"].as_i64().unwrap();
    let discount = report["discount_minor"].as_i64().unwrap();
    let total = report["total_minor"].as_i64().unwrap();
    assert_eq!(tax, 0);
    assert_eq!(discount, 0);
    assert_eq!(gaming + product + tax - discount, total);
    assert_eq!(report["sales_revenue_minor"], gaming + product);
    assert_eq!(report["paid_orders"], 1);
    let other = reports::sales_summary(&pool, Some("b2"), "1970-01-01T00:00:00Z", "2099-01-01T00:00:00Z")
        .await
        .unwrap();
    assert_eq!(other["paid_orders"], 0);

    backup::mark_restore_reconcile(&pool).await.unwrap();
    let recon: i64 =
        sqlx::query_scalar("SELECT restore_reconciliation_required FROM sync_state WHERE id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(recon, 1, "restore must gate pull-before-push");
}
