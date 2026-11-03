//! The schedule of an activity, read from PostgreSQL on every access (ADR-0020)

use content::{Schedule, Window};
use contracts::Id;
use sqlx::PgPool;

// An activity without a row gets no window: closed by default
pub(super) async fn read(pool: &pgPool, course: &Id, activity: &Id) -> sqlx::Result<Schedule> {
    todo!()
}
