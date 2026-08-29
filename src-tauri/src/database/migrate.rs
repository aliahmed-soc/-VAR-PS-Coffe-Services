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
    for line in sql.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("--") {
            continue;
        }
        buf.push_str(line);
        buf.push('\n');
        if trimmed.ends_with(';') {
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
}
