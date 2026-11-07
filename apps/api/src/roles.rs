//! Roles held in a course, read from PostgreSQL on every access (ADR-0012)

use contracts::Id;
use sqlx::PgPool;

pub(super) async fn has_role(pool: &PgPool, course: &Id, account: &str) -> sqlx::Result<bool> {
    sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM course_roles WHERE course = $1::text AND account = $2) AS "has_role!""#,
        course.as_str(),
        account
    )
    .fetch_one(pool)
    .await
}
