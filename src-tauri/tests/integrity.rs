use playstation_cafe_lib::database;
use playstation_cafe_lib::dev;
use playstation_cafe_lib::domain::{gaming, inventory, orders, payments};

#[tokio::test]
async fn migrations_and_full_sale_flow() {
    let pool = database::open_memory().await.expect("migrate");
    dev::seed_two_branches(&pool).await.expect("seed");

    let start = gaming::start_session(&pool, "b1", "d1", "s-ps1", "u-c1")
        .await
        .expect("start");
    let session_id = start["session_id"].as_str().unwrap().to_string();
    let order_id = start["order_id"].as_str().unwrap().to_string();

    let occupied = gaming::start_session(&pool, "b1", "d1", "s-ps1", "u-c1").await;
    assert!(occupied.is_err(), "second active session must fail");

    inventory::add_product_to_order(&pool, "b1", "d1", &order_id, "p-coke", 2, "u-c1")
        .await
        .expect("add coke");
    assert_eq!(inventory::stock(&pool, "b1", "p-coke").await.unwrap(), 48);
    assert_eq!(inventory::stock(&pool, "b2", "p-coke").await.unwrap(), 80);

    let denied = inventory::add_product_to_order(&pool, "b1", "d1", &order_id, "p-coke", 100, "u-c1").await;
    assert!(denied.is_err(), "negative stock must be blocked");

    let stop = gaming::stop_session(&pool, "b1", "d1", &session_id, "u-c1")
        .await
        .expect("stop");
    let again = gaming::stop_session(&pool, "b1", "d1", &session_id, "u-c1")
        .await
        .expect("idempotent stop");
    assert_eq!(stop["final_charge_minor"], again["final_charge_minor"]);

    let paid = payments::take_cash(&pool, "b1", "d1", &order_id, 20_000, "u-c1")
        .await
        .expect("pay");
    assert!(paid["receipt_snapshot"].is_object());
    let twice = payments::take_cash(&pool, "b1", "d1", &order_id, 20_000, "u-c1").await;
    assert!(twice.is_err(), "double payment must fail");

    let rev = payments::reverse_payment(&pool, "b1", "d1", &order_id, "u-admin", "wrong tender")
        .await
        .expect("reverse");
    assert_eq!(rev["order_id"], order_id);
    let order = orders::get_order(&pool, "b1", &order_id).await.unwrap();
    assert_eq!(order["status"], "checkout_pending");

    payments::take_cash(&pool, "b1", "d1", &order_id, 15_000, "u-c1")
        .await
        .expect("repay");

    let tax_row: (i64, i64, i64, i64, Option<String>) = sqlx::query_as(
        "SELECT tax_minor, tax_rate_bps, subtotal_minor, total_minor, receipt_snapshot FROM orders WHERE id = ?",
    )
    .bind(&order_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(tax_row.0, 0, "tax defaults to zero");
    assert_eq!(tax_row.1, 0);
    assert_eq!(tax_row.2, tax_row.3, "MVP subtotal_minor = total_minor");
    let snap: serde_json::Value = serde_json::from_str(tax_row.4.as_deref().unwrap()).unwrap();
    assert_eq!(snap["tax_minor"], 0);
    assert_eq!(snap["tax_rate_bps"], 0);
    assert_eq!(
        playstation_cafe_lib::domain::tax::replay_tax(&snap, 1400).unwrap().tax_minor,
        0,
        "sync replay must not recalculate tax"
    );
    let mutate = sqlx::query("UPDATE orders SET tax_minor = 99 WHERE id = ?")
        .bind(&order_id)
        .execute(&pool)
        .await;
    assert!(mutate.is_err(), "tax fields are immutable once paid");
    let negative = sqlx::query(
        "INSERT INTO orders (id, branch_id, order_type, status, currency_code, opened_by, opened_at, tax_minor)
         VALUES ('neg-tax','b1','pos','open','EGP','u-c1', '2026-01-01T00:00:00Z', -1)",
    )
    .execute(&pool)
    .await;
    assert!(negative.is_err(), "negative tax is rejected");

    let pending: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sync_outbox WHERE sync_status = 'pending'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(pending >= 4, "outbox must record domain events, got {pending}");

    let seqs: Vec<i64> = sqlx::query_scalar("SELECT sequence FROM sync_outbox WHERE device_id = 'd1' ORDER BY sequence")
        .fetch_all(&pool)
        .await
        .unwrap();
    for pair in seqs.windows(2) {
        assert_eq!(pair[1], pair[0] + 1, "local_sequence must be contiguous");
    }
}
