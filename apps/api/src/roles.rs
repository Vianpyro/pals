//! Roles held in an offering, read from PostgreSQL on every access (ADR-0012)

use contracts::Id;
use sqlx::PgPool;

pub(super) async fn course(
    pool: &PgPool,
    offering: &Id,
    account: &str,
) -> sqlx::Result<Option<String>> {
    sqlx::query_scalar!(
        r#"
            SELECT offerings.course::text AS "course!"
            FROM offering_roles
            JOIN offerings ON offerings.id = offering_roles.offering
            WHERE offering_roles.offering = $1::text AND offering_roles.account = $2
        "#,
        offering.as_str(),
        account
    )
    .fetch_optional(pool)
    .await
}
