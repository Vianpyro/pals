//! A student's runs: one followed live, and their history on an exercise, newest first (ADR-0030)

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use contracts::{Id, Run, RunStatus};
use sqlx::{PgPool, types::time::OffsetDateTime};
use uuid::Uuid;

use super::{kind::JobKind, session::Account};

#[derive(Debug, sqlx::Type)]
#[sqlx(type_name = "job_state", rename_all = "lowercase")]
enum JobState {
    Waiting,
    Running,
    Done,
    Replaced,
    Failed,
}

/// What the student sees of a job. None for a done job without a result, which the
/// `jobs_result_when_done` constraint forbids: a broken database is a 500 not a guess
fn status(state: JobState, passed: Option<bool>) -> Option<RunStatus> {
    Some(match state {
        JobState::Waiting => RunStatus::Waiting,
        JobState::Running => RunStatus::Running,
        JobState::Done => RunStatus::Done { passed: passed? },
        JobState::Replaced => RunStatus::Replaced,
        JobState::Failed => RunStatus::Failed,
    })
}

fn internal_error(context: &str, error: impl std::fmt::Display) -> StatusCode {
    eprintln!("api: {context}: {error}");
    StatusCode::INTERNAL_SERVER_ERROR
}

fn run(
    id: Uuid,
    kind: JobKind,
    state: JobState,
    passed: Option<bool>,
    enqueued_at: OffsetDateTime,
) -> Result<Run, StatusCode> {
    let status = status(state, passed)
        .ok_or_else(|| internal_error("runs", format!("job {id} is done without a result")))?;
    Ok(Run {
        id,
        kind: kind.into(),
        status,
        enqueued_at,
    })
}

pub(super) async fn one(pool: &PgPool, id: Uuid, author: &str) -> Result<Option<Run>, StatusCode> {
    sqlx::query!(
        r#"
            SELECT id AS "id!", kind AS "kind!: JobKind", state AS "state!: JobState", passed, enqueued_at AS "enqueued_at!"
            FROM runs WHERE id = $1 AND author = $2
        "#,
        id,
        author
    )
    .fetch_optional(pool)
    .await
    .map_err(|error| internal_error("run", error))?
    .map(|row| run(row.id, row.kind, row.state, row.passed, row.enqueued_at))
    .transpose()
}

pub(super) async fn latest(pool: &PgPool, author: &str) -> Result<Vec<Run>, StatusCode> {
    sqlx::query!(
        r#"
            SELECT DISTINCT ON (course, exercise)
                id AS "id!", kind AS "kind!: JobKind", state AS "state!: JobState", passed, enqueued_at AS "enqueued_at!"
                FROM runs WHERE author = $1
                ORDER BY course, exercise, enqueued_at DESC, id DESC
        "#,
        author
    )
    .fetch_all(pool)
    .await
    .map_err(|error| internal_error("latest runs", error))?
    .into_iter()
    .map(|row| run(row.id, row.kind, row.state, row.passed, row.enqueued_at))
    .collect()
}

pub(super) async fn progress(
    Account(author): Account,
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
) -> Result<Json<RunStatus>, StatusCode> {
    // Someone else's job is not found rather than forbidden: a 403 would confirm it exists
    let run = one(&pool, id, &author)
        .await?
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(run.status))
}

// No gate: the student's own runs, not content; they stay readable once the exercise closes
pub(super) async fn history(
    Account(author): Account,
    State(pool): State<PgPool>,
    Path((course, exercise)): Path<(Id, Id)>,
) -> Result<Json<Vec<Run>>, StatusCode> {
    sqlx::query!(
        r#"
            SELECT id AS "id!", kind AS "kind!: JobKind", state AS "state!: JobState", passed, enqueued_at AS "enqueued_at!"
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
    .map_err(|error| internal_error("history", error))?
    .into_iter()
    .map(|row| run(row.id, row.kind, row.state, row.passed, row.enqueued_at))
    .collect::<Result<_, _>>()
    .map(Json)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_done_job_has_a_verdict() {
        assert_eq!(
            status(JobState::Done, Some(true)),
            Some(RunStatus::Done { passed: true })
        );
        assert_eq!(status(JobState::Waiting, None), Some(RunStatus::Waiting));
    }

    #[test]
    fn a_done_job_without_a_result_is_refused() {
        assert_eq!(status(JobState::Done, None), None);
    }
}
