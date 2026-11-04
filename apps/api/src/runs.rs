//! A student's past runs on an exercise, newest first (ADR-0030)

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use contracts::Id;
use serde::Serialize;
use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

use super::session::Account;

#[derive(Serialize)]
pub(super) struct Run {
    id: Uuid,
    state: String,
    passed: Option<bool>,
    #[serde(with = "time::serde::rfc3339")]
    enqueued_at: OffsetDateTime,
}

// No gate: the student's own runs, not content; they stay readable once the exercise closes
pub(super) async fn history(
    Account(author): Account,
    State(pool): State<PgPool>,
    Path((course, exercise)): Path<(Id, Id)>,
) -> Result<Json<Vec<Run>>, StatusCode> {
    let runs = sqlx::query_as!(
        Run,
        r#"
            SELECT id AS "id!", state::text AS "state!", passed, enqueued_at AS "enqueued_at!"
            FROM runs
            WHERE author = $1 AND course = $2::text AND exercise = $3::text
            ORDER BY enqueued_at DESC, id DESC
        "#,
        author,
        course.as_str(),
        exercise.as_str()
    )
    .fetch_all(&pool)
    .await
    .map_err(|error| {
        eprintln!("api: history: {error}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(runs))
}
