// receipt_number carries a UNIQUE index with no branch in it, so the counter has
// to be per branch, per day, and never reused. Both cases below failed with
// "UNIQUE constraint failed: orders.receipt_number" when the counter was a count
// of orders holding a receipt. The first was reproduced during physical UAT: a
// cashier could not close any further sale on a day that had seen one repayment.
use playstation_cafe_lib::database;
use playstation_cafe_lib::dev;
use playstation_cafe_lib::domain::{inventory, orders, payments};

async fn paid_receipt(pool: &sqlx::SqlitePool, order_id: &str) -> String {
    sqlx::query_scalar("SELECT receipt_number FROM orders WHERE id = ?")
        .bind(order_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn walk_in_with_one_drink(pool: &sqlx::SqlitePool, branch: &str, device: &str) -> String {
    let order = orders::open_pos_order(pool, branch, device, "u-c1").await.unwrap();
    let order_id = order["order_id"].as_str().unwrap().to_string();
    inventory::add_product_to_order(pool, branch, device, &order_id, "p-coke", 1, "u-c1")
        .await
        .unwrap();
    order_id
}

async fn add_branch_two_device(pool: &sqlx::SqlitePool) {
    sqlx::query(
        "INSERT OR IGNORE INTO devices (id, branch_id, name, device_key, is_active, paired_at)
         VALUES ('d2','b2','Cashier 2','dev-key-b2',1,?)",
    )
    .bind(chrono::Utc::now().to_rfc3339())
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn sale_after_a_repayment_gets_a_fresh_receipt_number() {
    let pool = database::open_memory().await.unwrap();
    dev::seed_two_branches(&pool).await.unwrap();

    let first = walk_in_with_one_drink(&pool, "b1", "d1").await;
    payments::take_cash(&pool, "b1", "d1", &first, 20_000, "u-c1")
        .await
        .expect("first sale");
    let original = paid_receipt(&pool, &first).await;

    payments::reverse_payment(&pool, "b1", "d1", &first, "u-admin", "cashier correction")
        .await
        .expect("reverse");
    payments::take_cash(&pool, "b1", "d1", &first, 20_000, "u-c1")
        .await
        .expect("repay");
    let repaid = paid_receipt(&pool, &first).await;
    assert_ne!(repaid, original, "a repayment must issue a new receipt");

    let second = walk_in_with_one_drink(&pool, "b1", "d1").await;
    payments::take_cash(&pool, "b1", "d1", &second, 20_000, "u-c1")
        .await
        .expect("the next sale of the day must still be closable after a repayment");
    let next = paid_receipt(&pool, &second).await;

    assert_ne!(next, repaid);
    assert_ne!(next, original);
    let distinct: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT receipt_number) FROM orders WHERE receipt_number IS NOT NULL",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(distinct, 2, "two paid orders hold two distinct receipts");
}

#[tokio::test]
async fn two_branches_paying_the_same_day_do_not_collide() {
    let pool = database::open_memory().await.unwrap();
    dev::seed_two_branches(&pool).await.unwrap();
    add_branch_two_device(&pool).await;

    let b1_order = walk_in_with_one_drink(&pool, "b1", "d1").await;
    payments::take_cash(&pool, "b1", "d1", &b1_order, 20_000, "u-c1")
        .await
        .expect("b1 sale");
    let b2_order = walk_in_with_one_drink(&pool, "b2", "d2").await;
    payments::take_cash(&pool, "b2", "d2", &b2_order, 20_000, "u-c1")
        .await
        .expect("b2 must be able to pay on the same day as b1");

    let b1_receipt = paid_receipt(&pool, &b1_order).await;
    let b2_receipt = paid_receipt(&pool, &b2_order).await;
    assert_ne!(b1_receipt, b2_receipt);
    assert!(b1_receipt.starts_with("B1-"), "got {b1_receipt}");
    assert!(b2_receipt.starts_with("B2-"), "got {b2_receipt}");
}

#[tokio::test]
async fn receipt_counter_advances_within_a_branch_day() {
    let pool = database::open_memory().await.unwrap();
    dev::seed_two_branches(&pool).await.unwrap();

    let mut numbers = Vec::new();
    for _ in 0..3 {
        let order = walk_in_with_one_drink(&pool, "b1", "d1").await;
        payments::take_cash(&pool, "b1", "d1", &order, 20_000, "u-c1")
            .await
            .unwrap();
        numbers.push(paid_receipt(&pool, &order).await);
    }
    let tails: Vec<&str> = numbers.iter().map(|n| &n[n.len() - 4..]).collect();
    assert_eq!(tails, ["0001", "0002", "0003"]);
}
