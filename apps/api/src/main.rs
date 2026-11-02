//! Sign-in, sessions, offerings, submissions and verdict notifications.

use std::{env, error::Error, sync::Arc};

use axum::{
    Json, Router,
    extract::{FromRef, Path, State},
    http::{HeaderName, StatusCode, header},
    routing::{get, post},
};
use contracts::Id;
use directory::{Directory, Identity, SignInError};
use serde::{Deserialize, Serialize};
use session::Account;
use sqlx::{PgPool, migrate::Migrator};
use uuid::Uuid;

mod directory;
mod session;

static MIGRATOR: Migrator = sqlx::migrate!("../../db/migrations");

const CONTRACT: i32 = 1;
const MAX_SOURCE: usize = 1024 * 1024;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let database = env::var("DATABASE_URL")?;
    let pool = PgPool::connect(&database).await?;

    match env::args().nth(1).as_deref() {
        None => {
            let directory = Directory::from_env()?;
            let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
            axum::serve(listener, app(pool, directory)).await?;
        }
        Some("migrate") => MIGRATOR.run(&pool).await?,
        Some(command) => return Err(format!("unknown command: {command}").into()),
    }

    Ok(())
}

#[derive(Clone)]
struct AppState {
    pool: PgPool,
    directory: Arc<Directory>,
}

impl FromRef<AppState> for PgPool {
    fn from_ref(state: &AppState) -> Self {
        state.pool.clone()
    }
}

impl FromRef<AppState> for Arc<Directory> {
    fn from_ref(state: &AppState) -> Self {
        Arc::clone(&state.directory)
    }
}

fn app(pool: PgPool, directory: Directory) -> Router {
    let state = AppState {
        pool,
        directory: Arc::new(directory),
    };
    Router::new()
        .route("/health", get(|| async { StatusCode::OK }))
        .route("/sessions", post(sign_in))
        .route("/submissions", post(submit))
        .route("/submissions/{id}", get(progress))
        .with_state(state)
}

// No Debug: it holds the password
#[derive(Deserialize)]
struct Credentials {
    id: String,
    password: String,
}

async fn sign_in(
    State(pool): State<PgPool>,
    State(directory): State<Arc<Directory>>,
    Json(credentials): Json<Credentials>,
) -> Result<([(HeaderName, String); 1], Json<Identity>), StatusCode> {
    let identity = directory
        .sign_in(&credentials.id, &credentials.password)
        .await
        .map_err(|error| match error {
            SignInError::Refused => StatusCode::UNAUTHORIZED,
            SignInError::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
        })?;

    let token = session::open(&pool, &identity.id).await.map_err(|error| {
        eprintln!("api: sign-in: {error}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let cookie = format!(
        "{}={token}; Path=/; Secure; HttpOnly; SameSite=Lax",
        session::COOKIE
    );
    Ok(([(header::SET_COOKIE, cookie)], Json(identity)))
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

async fn submit(
    _account: Account,
    State(pool): State<PgPool>,
    Json(submission): Json<Submission>,
) -> Result<(StatusCode, Json<Submitted>), StatusCode> {
    if submission.source.is_empty() || submission.source.len() > MAX_SOURCE {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    let id = sqlx::query_scalar!(
        r#"
        INSERT INTO jobs (contract, course, exercise, language_pack, source)
        VALUES ($1, $2::text, $3::text, $4::text, $5)
        RETURNING id AS "id!"
        "#,
        CONTRACT,
        submission.course.as_str(),
        submission.exercise.as_str(),
        submission.language_pack.as_str(),
        submission.source
    )
    .fetch_one(&pool)
    .await
    .map_err(|error| {
        eprintln!("api: enqueue: {error}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok((StatusCode::ACCEPTED, Json(Submitted { id })))
}

#[derive(Serialize)]
struct Progress {
    state: String,
    passed: Option<bool>,
}

async fn progress(
    _account: Account,
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
) -> Result<Json<Progress>, StatusCode> {
    let progress = sqlx::query_as!(
        Progress,
        r#"
            SELECT state::text AS "state!", (result -> 'cases') <@ '["passed"]' AS "passed?"
            FROM jobs WHERE id = $1
        "#,
        id
    )
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
        response::Response,
    };
    use tower::ServiceExt;

    const RIGHT_PASSWORD: &str = r#"{"id": "etudiant1", "password": "log791-dev"}"#;
    const WRONG_PASSWORD: &str = r#"{"id": "etudiant1", "password": "wrong"}"#;

    async fn sign_in_with(pool: &PgPool, directory: Directory, body: &'static str) -> Response {
        let request = Request::post("/sessions")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body))
            .unwrap();
        app(pool.clone(), directory).oneshot(request).await.unwrap()
    }

    async fn signed_in(pool: &PgPool) -> String {
        let token = session::open(pool, "etudiant1").await.unwrap();
        format!("{}={token}", session::COOKIE)
    }

    async fn sessions(pool: &PgPool) -> i64 {
        sqlx::query_scalar("SELECT count(*) FROM sessions")
            .fetch_one(pool)
            .await
            .unwrap()
    }

    async fn post_submission(pool: &PgPool, cookie: &str, body: &'static str) -> StatusCode {
        let request = Request::post("/submissions")
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::COOKIE, cookie)
            .body(Body::from(body))
            .unwrap();
        app(pool.clone(), Directory::unreachable())
            .oneshot(request)
            .await
            .unwrap()
            .status()
    }

    async fn get_submission(pool: &PgPool, cookie: &str, id: &str) -> (StatusCode, String) {
        let request = Request::get(format!("/submissions/{id}"))
            .header(header::COOKIE, cookie)
            .body(Body::empty())
            .unwrap();
        let response = app(pool.clone(), Directory::unreachable())
            .oneshot(request)
            .await
            .unwrap();
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
        let response = app(pool, Directory::unreachable())
            .oneshot(request)
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn empty_password_is_unauthorized() {
        let pool = PgPool::connect_lazy("postgres://unused").unwrap();
        let body = r#"{"id": "etudiant1", "password": ""}"#;
        let response = sign_in_with(&pool, Directory::unreachable(), body).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn unreachable_directory_is_unavailable() {
        let pool = PgPool::connect_lazy("postgres://unused").unwrap();
        let response = sign_in_with(&pool, Directory::unreachable(), RIGHT_PASSWORD).await;
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn submission_is_queued(pool: PgPool) {
        let body = r#"{"course": "log200", "exercise": "sum", "language_pack": "python", "source": "print(0)"}"#;
        assert_eq!(
            post_submission(&pool, &signed_in(&pool).await, body).await,
            StatusCode::ACCEPTED
        );
        assert_eq!(waiting_jobs(&pool).await, 1);
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn invalid_identifier_is_rejected(pool: PgPool) {
        let body = r#"{"course": "log200", "exercise": "../sum", "language_pack": "python", "source": "print(0)"}"#;
        assert_eq!(
            post_submission(&pool, &signed_in(&pool).await, body).await,
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
            post_submission(&pool, &signed_in(&pool).await, body).await,
            StatusCode::UNPROCESSABLE_ENTITY
        );
        assert_eq!(waiting_jobs(&pool).await, 0);
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn malformed_id_is_a_bad_request(pool: PgPool) {
        assert_eq!(
            get_submission(&pool, &signed_in(&pool).await, "not-a-uuid")
                .await
                .0,
            StatusCode::BAD_REQUEST
        );
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn unknown_submission_is_not_found(pool: PgPool) {
        assert_eq!(
            get_submission(&pool, &signed_in(&pool).await, &Uuid::nil().to_string()).await,
            (StatusCode::NOT_FOUND, String::new())
        );
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn queued_submission_is_waiting(pool: PgPool) {
        let job = enqueue(&pool).await;
        assert_eq!(
            get_submission(&pool, &signed_in(&pool).await, &job.to_string()).await,
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
            get_submission(&pool, &signed_in(&pool).await, &failed.to_string()).await,
            (
                StatusCode::OK,
                r#"{"state":"done","passed":false}"#.to_string()
            )
        );

        let passed = enqueue(&pool).await;
        judge(&pool, passed, "{passed,passed}").await;
        assert_eq!(
            get_submission(&pool, &signed_in(&pool).await, &passed.to_string()).await,
            (
                StatusCode::OK,
                r#"{"state":"done","passed":true}"#.to_string()
            )
        );
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn submission_without_session_is_unauthorized(pool: PgPool) {
        let body = r#"{"course": "log200", "exercise": "sum", "language_pack": "python", "source": "print(0)"}"#;
        assert_eq!(
            post_submission(&pool, "", body).await,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(waiting_jobs(&pool).await, 0);
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn expired_session_is_unauthorized(pool: PgPool) {
        let cookie = signed_in(&pool).await;
        sqlx::query(
            "UPDATE sessions SET opened_at = now() - interval '2 days', expires_at = now() - interval '1 day'",
        )
        .execute(&pool).await.unwrap();

        let body = r#"{"course": "log200", "exercise": "sum", "language_pack": "python", "source": "print(0)"}"#;
        assert_eq!(
            post_submission(&pool, &cookie, body).await,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(waiting_jobs(&pool).await, 0);
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn session_cookie_is_found_among_others(pool: PgPool) {
        let cookie = format!("theme=dark; {}", signed_in(&pool).await);
        let body = r#"{"course": "log200", "exercise": "sum", "language_pack": "python", "source": "print(0)"}"#;
        assert_eq!(
            post_submission(&pool, &cookie, body).await,
            StatusCode::ACCEPTED
        );
        assert_eq!(waiting_jobs(&pool).await, 1);
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL and LDAP"]
    async fn right_password_opens_a_session(pool: PgPool) {
        let response = sign_in_with(&pool, Directory::from_env().unwrap(), RIGHT_PASSWORD).await;
        assert_eq!(response.status(), StatusCode::OK);

        let cookie = response.headers()[header::SET_COOKIE].to_str().unwrap();
        let (token, attributes) = cookie
            .strip_prefix("__Host-session=")
            .and_then(|rest| rest.split_once(';'))
            .unwrap();
        assert_eq!(attributes, " Path=/; Secure; HttpOnly; SameSite=Lax");
        assert_eq!(
            session::account(&pool, token).await.unwrap().as_deref(),
            Some("etudiant1")
        );

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(
            body,
            r#"{"id":"etudiant1","name":"Céline Dion","email":"etudiant1@log791.ca"}"#
        );
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL and LDAP"]
    async fn wrong_password_opens_no_session(pool: PgPool) {
        let response = sign_in_with(&pool, Directory::from_env().unwrap(), WRONG_PASSWORD).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert!(!response.headers().contains_key(header::SET_COOKIE));
        assert_eq!(sessions(&pool).await, 0);
    }
}
