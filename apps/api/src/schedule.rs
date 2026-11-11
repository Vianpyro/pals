//! The schedule of an activity, read from PostgreSQL on every access (ADR-0020)

use content::{Schedule, Window};
use contracts::Id;
use sqlx::PgPool;

// An activity without a row gets no window: closed by default
pub(super) async fn read(pool: &PgPool, offering: &Id, activity: &Id) -> sqlx::Result<Schedule> {
    let offering = sqlx::query!(
        "SELECT opens_at, closes_at FROM schedules WHERE offering = $1::text AND activity = $2::text",
        offering.as_str(),
        activity.as_str()
    )
    .fetch_optional(pool)
    .await?
    .map(|row| Window {
        opens_at: row.opens_at.into(),
        closes_at: row.closes_at.map(Into::into),
    });

    Ok(Schedule {
        offering,
        ..Default::default()
    })
}
