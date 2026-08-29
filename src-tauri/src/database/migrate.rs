use sqlx::SqlitePool;

use crate::error::AppResult;

const MIGRATIONS: &[(&str, i64, &str)] = &[
    (
        "0001_init",
        1,
        include_str!("../../migrations/sqlite/0001_init.sql"),
    ),
    (
        "0002_seed_payment_methods",
        2,
        include_str!("../../migrations/sqlite/0002_seed_payment_methods.sql"),
    ),
];

pub async fn apply(pool: &SqlitePool) -> AppResult<()> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY NOT NULL,
            name TEXT NOT NULL,
            applied_at TEXT NOT NULL
        )",
    )
    .execute(pool)
    .await?;

    for (name, version, sql) in MIGRATIONS {
        let exists: Option<i64> =
            sqlx::query_scalar("SELECT version FROM schema_migrations WHERE version = ?")
                .bind(version)
                .fetch_optional(pool)
                .await?;
        if exists.is_some() {
            continue;
        }
        let mut tx = pool.begin().await?;
        for statement in split_sql(sql) {
            if statement.trim().is_empty() {
                continue;
            }
            sqlx::query(&statement).execute(&mut *tx).await?;
        }
        sqlx::query("INSERT INTO schema_migrations (version, name, applied_at) VALUES (?, ?, ?)")
            .bind(version)
            .bind(name)
            .bind(chrono::Utc::now().to_rfc3339())
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
    }
    Ok(())
}

fn split_sql(sql: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut buf = String::new();
    let mut begin_depth = 0;
    for line in sql.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("--") {
            continue;
        }
        let upper = trimmed.to_ascii_uppercase();
        if upper == "BEGIN" || upper.starts_with("BEGIN ") {
            begin_depth += 1;
        }
        buf.push_str(line);
        buf.push('\n');
        if upper == "END;" || upper.ends_with(" END;") {
            begin_depth = begin_depth.saturating_sub(1);
        }
        if trimmed.ends_with(';') && begin_depth == 0 {
            out.push(std::mem::take(&mut buf));
        }
    }
    if !buf.trim().is_empty() {
        out.push(buf);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::split_sql;

    #[test]
    fn splits_on_semicolons() {
        let parts = split_sql("CREATE TABLE a (id INT);\nCREATE TABLE b (id INT);");
        assert_eq!(parts.len(), 2);
    }

    #[test]
    fn keeps_trigger_body_together() {
        let sql = "CREATE TRIGGER t\nBEFORE UPDATE ON orders\nBEGIN\n  SELECT RAISE(ABORT, 'paid_tax_immutable');\nEND;\nCREATE TABLE a (id INT);";
        let parts = split_sql(sql);
        assert_eq!(parts.len(), 2);
        assert!(parts[0].contains("CREATE TRIGGER"));
        assert!(parts[0].contains("END;"));
        assert!(parts[1].contains("CREATE TABLE a"));
    }

    #[test]
    fn init_migration_keeps_orders_and_tax_trigger_intact() {
        let sql = include_str!("../../migrations/sqlite/0001_init.sql");
        let parts = split_sql(sql);
        let orders = parts
            .iter()
            .find(|s| s.contains("CREATE TABLE orders"))
            .expect("orders table");
        assert!(
            orders.contains("amount_paid_minor"),
            "amount_paid_minor must be a column in CREATE TABLE orders"
        );
        assert!(
            orders
                .contains("CHECK (subtotal_minor = product_subtotal_minor + gaming_subtotal_minor)"),
            "subtotal identity must remain on orders"
        );
        let amount_at = orders.find("amount_paid_minor").unwrap();
        let check_at = orders
            .find("CHECK (subtotal_minor = product_subtotal_minor + gaming_subtotal_minor)")
            .unwrap();
        assert!(amount_at < check_at, "columns must precede table CHECKs");
        let trigger = parts
            .iter()
            .find(|s| s.contains("CREATE TRIGGER orders_paid_tax_immutable"))
            .expect("tax trigger");
        assert!(trigger.contains("paid_tax_immutable"));
        assert!(trigger.contains("END;"));
    }
}
