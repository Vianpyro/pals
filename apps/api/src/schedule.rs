//! The schedule of an activity, read from PostgreSQL on every access (ADR-0020)

use content::{Schedule, Window};
use contracts::Id;
use sqlx::PgPool;

// An activity without a row gets no window: closed by default
pub(super) async fn read(
    pool: &PgPool,
    offering: &Id,
    activity: &Id,
    account: &str,
) -> sqlx::Result<Schedule> {
    let rows = sqlx::query!(
        r#"
            SELECT
                account IS NOT NULL as "extension!",
                group_id IS NOT NULL AS "group!",
                opens_at,
                closes_at
            FROM schedules
            WHERE offering = $1::text AND activity = $2::text
                AND (
                    account = $3::text
                    OR group_id = (
                        SELECT group_id FROM group_members
                        WHERE offering = $1::text AND account = $3::text
                    )
                    OR (group_id IS NULL AND account IS NULL)
                )
        "#,
        offering.as_str(),
        activity.as_str(),
        account
    )
    .fetch_all(pool)
    .await?;

    let mut schedule = Schedule::default();
    for row in rows {
        let window = Some(Window {
            opens_at: row.opens_at.into(),
            closes_at: row.closes_at.map(Into::into),
        });
        match (row.extension, row.group) {
            (true, _) => schedule.extension = window,
            (false, true) => schedule.group = window,
            (false, false) => schedule.offering = window,
        }
    }

    Ok(schedule)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;

    // In log200, alice and bob are in g01 and carol in g02; dave is in g01 of log121 only
    async fn groups(pool: &PgPool) {
        sqlx::raw_sql(
            r#"
                INSERT INTO groups VALUES ('log200', 'g01'), ('log200', 'g02'), ('log121', 'g01');
                INSERT INTO group_members VALUES
                    ('log200', 'alice', 'g01'),
                    ('log200', 'bob', 'g01'),
                    ('log200', 'carol', 'g02'),
                    ('log121', 'dave', 'g01');
            "#,
        )
        .execute(pool)
        .await
        .unwrap();
    }

    // Opened an hour ago, or opens in an hour
    async fn schedule(pool: &PgPool, group: Option<&str>, account: Option<&str>, open: bool) {
        let opens_in = if open { "-1 hour" } else { "1 hour" };
        sqlx::query(
            r#"
                INSERT INTO schedules (offering, activity, group_id, account, opens_at)
                VALUES ('log200', 'sum', $1, $2, now() + $3::interval)
            "#,
        )
        .bind(group)
        .bind(account)
        .bind(opens_in)
        .execute(pool)
        .await
        .unwrap();
    }

    async fn is_open(pool: &PgPool, account: &str) -> bool {
        let offering = "log200".parse().unwrap();
        let sum = "sum".parse().unwrap();
        read(pool, &offering, &sum, account)
            .await
            .unwrap()
            .is_open(SystemTime::now())
    }

    #[sqlx::test(
        migrations = "../../db/migrations",
        fixtures(path = "../../../db/fixtures", scripts("offerings"))
    )]
    #[ignore = "needs PostgreSQL"]
    async fn open_for_one_group_closed_for_another(pool: PgPool) {
        groups(&pool).await;
        schedule(&pool, Some("g01"), None, true).await;
        schedule(&pool, Some("g02"), None, false).await;
        assert!(is_open(&pool, "alice").await);
        assert!(!is_open(&pool, "carol").await);
    }

    #[sqlx::test(
        migrations = "../../db/migrations",
        fixtures(path = "../../../db/fixtures", scripts("offerings"))
    )]
    #[ignore = "needs PostgreSQL"]
    async fn group_row_wins_over_offering_row(pool: PgPool) {
        groups(&pool).await;
        schedule(&pool, None, None, true).await;
        schedule(&pool, Some("g01"), None, false).await;
        assert!(!is_open(&pool, "alice").await);
        assert!(is_open(&pool, "carol").await);
    }

    #[sqlx::test(
        migrations = "../../db/migrations",
        fixtures(path = "../../../db/fixtures", scripts("offerings"))
    )]
    #[ignore = "needs PostgreSQL"]
    async fn extension_wins_over_group_row(pool: PgPool) {
        groups(&pool).await;
        schedule(&pool, Some("g01"), None, false).await;
        schedule(&pool, None, Some("alice"), true).await;
        assert!(is_open(&pool, "alice").await);
        assert!(!is_open(&pool, "bob").await);
    }

    #[sqlx::test(
        migrations = "../../db/migrations",
        fixtures(path = "../../../db/fixtures", scripts("offerings"))
    )]
    #[ignore = "needs PostgreSQL"]
    async fn offering_row_applies_without_a_group(pool: PgPool) {
        schedule(&pool, None, None, true).await;
        assert!(is_open(&pool, "erin").await);
    }

    #[sqlx::test(
        migrations = "../../db/migrations",
        fixtures(path = "../../../db/fixtures", scripts("offerings"))
    )]
    #[ignore = "needs PostgreSQL"]
    async fn a_group_of_another_offering_does_not_count(pool: PgPool) {
        groups(&pool).await;
        schedule(&pool, Some("g01"), None, true).await;
        assert!(!is_open(&pool, "dave").await);
    }
}
