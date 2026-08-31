// Voiding a whole ticket has to put its lines back on the shelf. void_open_order
// only flipped the order to 'void', so the lines stayed 'active' and the units it
// had already deducted were never credited back. Reproduced during physical UAT:
// a drink added to a mistyped walk-in ticket stayed missing from stock after the
// cashier voided the ticket, and the cloud agreed on the wrong number.
use playstation_cafe_lib::database;
use playstation_cafe_lib::dev;
use playstation_cafe_lib::domain::{inventory, orders, payments};

    10|async fn stock(pool: &sqlx::SqlitePool, branch: &str, product: &str) -> i64 {
    sqlx::query_scalar(
        "SELECT quantity_on_hand FROM inventory_balances WHERE branch_id = ? AND product_id = ?",
    )
    .bind(branch)
    .bind(product)
    .fetch_one(pool)
    .await
    .unwrap()
}

    20|async fn item_statuses(pool: &sqlx::SqlitePool, order_id: &str) -> Vec<String> {
    sqlx::query_scalar("SELECT status FROM order_items WHERE order_id = ? ORDER BY id")
        .bind(order_id)
        .fetch_all(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn voiding_a_ticket_returns_its_lines_to_stock() {
    30|    let pool = database::open_memory().await.unwrap();
    dev::seed_two_branches(&pool).await.unwrap();
    let opening = stock(&pool, "b1", "p-coke").await;

    let order = orders::open_pos_order(&pool, "b1", "d1", "u-c1").await.unwrap();
    let order_id = order["order_id"].as_str().unwrap().to_string();
    inventory::add_product_to_order(&pool, "b1", "d1", &order_id, "p-coke", 2, "u-c1")
        .await
        .unwrap();
    assert_eq!(stock(&pool, "b1", "p-coke").await, opening - 2);
    40|
    orders::void_open_order(&pool, "b1", "d1", &order_id, "u-c1", "mistyped ticket")
        .await
        .expect("void");

    assert_eq!(
        stock(&pool, "b1", "p-coke").await,
        opening,
        "a voided ticket sold nothing, so its stock must come back"
    );
    assert_eq!(item_statuses(&pool, &order_id).await, ["voided"]);
    50|
    let (status, product_subtotal, total): (String, i64, i64) = sqlx::query_as(
        "SELECT status, product_subtotal_minor, total_minor FROM orders WHERE id = ?",
    )
    .bind(&order_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "void");
    assert_eq!(product_subtotal, 0, "a void ticket carries no product value");
    60|    assert_eq!(total, 0);
}

#[tokio::test]
async fn voiding_a_ticket_records_one_sale_void_movement_per_line() {
    let pool = database::open_memory().await.unwrap();
    dev::seed_two_branches(&pool).await.unwrap();

    let order = orders::open_pos_order(&pool, "b1", "d1", "u-c1").await.unwrap();
    let order_id = order["order_id"].as_str().unwrap().to_string();
    70|    inventory::add_product_to_order(&pool, "b1", "d1", &order_id, "p-coke", 1, "u-c1")
        .await
        .unwrap();
    inventory::add_product_to_order(&pool, "b1", "d1", &order_id, "p-chips", 3, "u-c1")
        .await
        .unwrap();

    orders::void_open_order(&pool, "b1", "d1", &order_id, "u-c1", "mistyped ticket")
        .await
        .unwrap();
    80|
    let movements: Vec<(String, i64, i64)> = sqlx::query_as(
        "SELECT product_id, quantity_delta, quantity_after FROM inventory_movements
         WHERE order_id = ? AND movement_type = 'sale_void' ORDER BY product_id",
    )
    .bind(&order_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(movements.len(), 2, "one credit per voided line");
    90|    for (product, delta, after) in &movements {
        assert!(*delta > 0, "a void credits stock back");
        assert_eq!(
            *after,
            stock(&pool, "b1", product).await,
            "the movement must close on the balance the void produced"
        );
    }

    // Every movement points at the order.voided event, so the ledger explains
   100|    // itself without joining through the order.
    let orphans: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM inventory_movements
         WHERE order_id = ? AND movement_type = 'sale_void' AND origin_event_id IS NULL",
    )
    .bind(&order_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(orphans, 0);
   110|}

#[tokio::test]
async fn voiding_an_empty_ticket_leaves_stock_alone() {
    let pool = database::open_memory().await.unwrap();
    dev::seed_two_branches(&pool).await.unwrap();
    let opening = stock(&pool, "b1", "p-coke").await;

    let order = orders::open_pos_order(&pool, "b1", "d1", "u-c1").await.unwrap();
    let order_id = order["order_id"].as_str().unwrap().to_string();
   120|    orders::void_open_order(&pool, "b1", "d1", &order_id, "u-c1", "opened by mistake")
        .await
        .unwrap();

    assert_eq!(stock(&pool, "b1", "p-coke").await, opening);
    let movements: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM inventory_movements WHERE order_id = ?")
            .bind(&order_id)
            .fetch_one(&pool)
            .await
            .unwrap();
   130|    assert_eq!(movements, 0);
}

#[tokio::test]
async fn a_line_voided_before_the_ticket_is_not_credited_twice() {
    let pool = database::open_memory().await.unwrap();
    dev::seed_two_branches(&pool).await.unwrap();
    let opening = stock(&pool, "b1", "p-coke").await;

    let order = orders::open_pos_order(&pool, "b1", "d1", "u-c1").await.unwrap();
   140|    let order_id = order["order_id"].as_str().unwrap().to_string();
    let item = inventory::add_product_to_order(&pool, "b1", "d1", &order_id, "p-coke", 1, "u-c1")
        .await
        .unwrap();
    let item_id = item["order_item_id"].as_str().unwrap().to_string();

    inventory::void_order_item(&pool, "b1", "d1", &item_id, "u-c1", "wrong item")
        .await
        .unwrap();
    assert_eq!(stock(&pool, "b1", "p-coke").await, opening);
   150|
    orders::void_open_order(&pool, "b1", "d1", &order_id, "u-c1", "abandoned")
        .await
        .unwrap();

    assert_eq!(
        stock(&pool, "b1", "p-coke").await,
        opening,
        "the line was already credited, so the ticket void must not credit it again"
    );
}
   160|
#[tokio::test]
async fn a_paid_ticket_still_cannot_be_voided() {
    let pool = database::open_memory().await.unwrap();
    dev::seed_two_branches(&pool).await.unwrap();

    let order = orders::open_pos_order(&pool, "b1", "d1", "u-c1").await.unwrap();
    let order_id = order["order_id"].as_str().unwrap().to_string();
    inventory::add_product_to_order(&pool, "b1", "d1", &order_id, "p-coke", 1, "u-c1")
        .await
   170|        .unwrap();
    payments::take_cash(&pool, "b1", "d1", &order_id, 20_000, "u-c1")
        .await
        .unwrap();
    let paid_stock = stock(&pool, "b1", "p-coke").await;

    orders::void_open_order(&pool, "b1", "d1", &order_id, "u-c1", "too late")
        .await
        .expect_err("a paid ticket needs a reversal, not a void");

   180|    assert_eq!(
        stock(&pool, "b1", "p-coke").await,
        paid_stock,
        "a refused void must not move stock"
    );
}
