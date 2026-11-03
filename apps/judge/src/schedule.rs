//! The schedule of an exercise, reread before judging it (ARD-0005, ADR-0020)

use content::{Schedule, Window};
use sqlx::PgConnection;

// An exercise without a row gets no window: its jobs are never judged
pub async fn read_schedule(
    connection: &mut PgConnection,
    course: &str,
    exercise: &str,
) -> sqlx::Result<Schedule> {
    let offering = sqlx::query!(
        "SELECT opens_at, closes_at FROM schedules WHERE course = $1::text AND activity = $2::text",
        course,
        exercise
    )
    .fetch_optional(connection)
    .await?
    .map(|row| Window {
        opens_at: row.opens_at.into(),
        closes_at: row.closes_at.map(Into::into),
    });

    Ok(Schedule {
        extension: offering,
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::PgPool;
    use std::time::SystemTime;

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn judge_reads_the_offering_window(pool: PgPool) {
        sqlx::query(
            "INSERT INTO schedules VALUES ('log200', 'sum', now() - interval '1 hour', NULL)",
        )
        .execute(&pool)
        .await
        .unwrap();

        // Tests connect as a superuser: read with the judge's privileges instead
        let mut transaction = pool.begin().await.unwrap();
        sqlx::query("SET LOCAL ROLE judge")
            .execute(&mut *transaction)
            .await
            .unwrap();

        let now = SystemTime::now();
        let sum = read_schedule(&mut transaction, "log200", "sum")
            .await
            .unwrap();
        let other = read_schedule(&mut transaction, "log200", "other")
            .await
            .unwrap();
        assert!(sum.is_open(now));
        assert!(!other.is_open(now));
    }
}
