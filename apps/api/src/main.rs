//! Sign-in, sessions, offerings, submissions and verdict notifications.

use std::{env, error::Error};

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
};
use contracts::Id;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool, migrate::Migrator};
use uuid::Uuid;

static MIGRATOR: Migrator = sqlx::migrate!("../../db/migrations");

const CONTRACT: i32 = 1;
const MAX_SOURCE: usize = 1024 * 1024;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let database = env::var("DATABASE_URL")?;
    let pool = PgPool::connect(&database).await?;

    match env::args().nth(1).as_deref() {
        None => {
            let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
            axum::serve(listener, app(pool)).await?;
        }
        Some("migrate") => MIGRATOR.run(&pool).await?,
        Some(command) => return Err(format!("unknown command: {command}").into()),
    }

    Ok(())
}

fn app(pool: PgPool) -> Router {
    Router::new()
        .route("/health", get(|| async { StatusCode::OK }))
        .route("/submissions", post(submit))
        .route("/submissions/{id}", get(progress))
        .with_state(pool)
}

#[derive(Deserialize)]
struct Submission {
    course: Id,
    exercise: Id,
    language_pack: Id,
    source: String,
}

#[derive(Serialize)]
struct Submitted {
    id: Uuid,
}

const ENQUEUE: &str = "
    INSERT INTO jobs (contract, course, exercise, language_pack, source)
    VALUES ($1, $2, $3, $4, $5)
    RETURNING id
";

async fn submit(
    State(pool): State<PgPool>,
    Json(submission): Json<Submission>,
) -> Result<(StatusCode, Json<Submitted>), StatusCode> {
    if submission.source.is_empty() || submission.source.len() > MAX_SOURCE {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    let id = sqlx::query_scalar(ENQUEUE)
        .bind(CONTRACT)
        .bind(submission.course.as_str())
        .bind(submission.exercise.as_str())
        .bind(submission.language_pack.as_str())
        .bind(submission.source)
        .fetch_one(&pool)
        .await
        .map_err(|error| {
            eprintln!("api: enqueue: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok((StatusCode::ACCEPTED, Json(Submitted { id })))
}

#[derive(Serialize, FromRow)]
struct Progress {
    state: String,
    passed: Option<bool>,
}

const PROGRESS: &str = r#"
    SELECT state::text, (result -> 'cases') <@ '["passed"]' AS passed
    FROM jobs WHERE id = $1
"#;

async fn progress(
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
) -> Result<Json<Progress>, StatusCode> {
    let progress = sqlx::query_as(PROGRESS)
        .bind(id)
        .fetch_optional(&pool)
        .await
        .map_err(|error| {
            eprintln!("api: progress: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    progress.map(Json).ok_or(StatusCode::NOT_FOUND)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, header},
    };
    use tower::ServiceExt;

    async fn post_submission(pool: &PgPool, body: &'static str) -> StatusCode {
        let request = Request::post("/submissions")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body))
            .unwrap();
        app(pool.clone()).oneshot(request).await.unwrap().status()
    }

    async fn get_submission(pool: &PgPool, id: &str) -> (StatusCode, String) {
        let request = Request::get(format!("/submissions/{id}"))
            .body(Body::empty())
            .unwrap();
        let response = app(pool.clone()).oneshot(request).await.unwrap();
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, String::from_utf8(body.to_vec()).unwrap())
    }

    async fn waiting_jobs(pool: &PgPool) -> i64 {
        sqlx::query_scalar("SELECT count(*) FROM jobs WHERE state = 'waiting'")
            .fetch_one(pool)
            .await
            .unwrap()
    }

    async fn enqueue(pool: &PgPool) -> Uuid {
        sqlx::query_scalar(
            "INSERT INTO jobs (contract, course, exercise, language_pack, source) VALUES (1, 'log200', 'sum', 'python', 'print(0)') RETURNING id",
        )
        .fetch_one(pool)
        .await
        .unwrap()
    }

    async fn judge(pool: &PgPool, job: Uuid, verdicts: &str) {
        sqlx::query("SELECT id FROM claim_job()")
            .execute(pool)
            .await
            .unwrap();
        let finished: bool = sqlx::query_scalar("SELECT finish_job($1, $2::case_verdict[])")
            .bind(job)
            .bind(verdicts)
            .fetch_one(pool)
            .await
            .unwrap();
        assert!(finished);
    }

    #[sqlx::test(migrations = false)]
    #[ignore = "needs PostgreSQL"]
    async fn migrations_apply_once(pool: PgPool) {
        MIGRATOR.run(&pool).await.unwrap();
        MIGRATOR.run(&pool).await.unwrap();
        assert_eq!(waiting_jobs(&pool).await, 0);
    }

    #[tokio::test]
    async fn health_is_ok() {
        let pool = PgPool::connect_lazy("postgres://unused").unwrap();
        let request = Request::get("/health").body(Body::empty()).unwrap();
        let response = app(pool).oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn submission_is_queued(pool: PgPool) {
        let body = r#"{"course": "log200", "exercise": "sum", "language_pack": "python", "source": "print(0)"}"#;
        assert_eq!(post_submission(&pool, body).await, StatusCode::ACCEPTED);
        assert_eq!(waiting_jobs(&pool).await, 1);
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn invalid_identifier_is_rejected(pool: PgPool) {
        let body = r#"{"course": "log200", "exercise": "../sum", "language_pack": "python", "source": "print(0)"}"#;
        assert_eq!(
            post_submission(&pool, body).await,
            StatusCode::UNPROCESSABLE_ENTITY
        );
        assert_eq!(waiting_jobs(&pool).await, 0);
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn empty_source_is_rejected(pool: PgPool) {
        let body =
            r#"{"course": "log200", "exercise": "sum", "language_pack": "python", "source": ""}"#;
        assert_eq!(
            post_submission(&pool, body).await,
            StatusCode::UNPROCESSABLE_ENTITY
        );
        assert_eq!(waiting_jobs(&pool).await, 0);
    }

    #[tokio::test]
    async fn malformed_id_is_a_bad_request() {
        let pool = PgPool::connect_lazy("postgres://unused").unwrap();
        assert_eq!(
            get_submission(&pool, "not-a-uuid").await.0,
            StatusCode::BAD_REQUEST
        );
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn unknown_submission_is_not_found(pool: PgPool) {
        assert_eq!(
            get_submission(&pool, &Uuid::nil().to_string()).await,
            (StatusCode::NOT_FOUND, String::new())
        );
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn queued_submission_is_waiting(pool: PgPool) {
        let job = enqueue(&pool).await;
        assert_eq!(
            get_submission(&pool, &job.to_string()).await,
            (
                StatusCode::OK,
                r#"{"state":"waiting","passed":null}"#.to_string()
            )
        );
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn judged_submission_shows_only_whether_all_passed(pool: PgPool) {
        let failed = enqueue(&pool).await;
        judge(&pool, failed, "{passed,wrong-answer}").await;
        assert_eq!(
            get_submission(&pool, &failed.to_string()).await,
            (
                StatusCode::OK,
                r#"{"state":"done","passed":false}"#.to_string()
            )
        );

        let passed = enqueue(&pool).await;
        judge(&pool, passed, "{passed,passed}").await;
        assert_eq!(
            get_submission(&pool, &passed.to_string()).await,
            (
                StatusCode::OK,
                r#"{"state":"done","passed":true}"#.to_string()
            )
        );
    }
}
