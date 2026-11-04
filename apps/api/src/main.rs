//! Sign-in, sessions, offerings, submissions and verdict notifications.

use std::{
    env,
    error::Error,
    fs,
    path::{self, PathBuf},
    sync::Arc,
};

use axum::{
    Json, Router,
    extract::{FromRef, Path, State},
    http::{HeaderMap, HeaderName, StatusCode, header},
    routing::{get, post},
};
use contracts::Id;
use directory::{Directory, Identity, SignInError};
use serde::{Deserialize, Serialize};
use session::Account;
use sqlx::{PgPool, migrate::Migrator};
use uuid::Uuid;

mod directory;
mod gate;
mod schedule;
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
            let courses = env::var_os("COURSES").ok_or("COURSES is not set")?;
            let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
            axum::serve(listener, app(pool, directory, courses.into())).await?;
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
    courses: Arc<path::Path>,
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

impl FromRef<AppState> for Arc<path::Path> {
    fn from_ref(state: &AppState) -> Self {
        Arc::clone(&state.courses)
    }
}

fn app(pool: PgPool, directory: Directory, courses: PathBuf) -> Router {
    let state = AppState {
        pool,
        directory: Arc::new(directory),
        courses: Arc::from(courses),
    };
    Router::new()
        .route(
            "/courses/{course}/exercises/{exercise}/statement",
            get(statement),
        )
        .route("/health", get(|| async { StatusCode::OK }))
        .route("/sessions", post(sign_in).delete(sign_out))
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

    let cookie = session::cookie(&token);
    Ok(([(header::SET_COOKIE, cookie)], Json(identity)))
}

async fn sign_out(
    State(pool): State<PgPool>,
    headers: HeaderMap,
) -> Result<(StatusCode, [(HeaderName, String); 1]), StatusCode> {
    if let Some(token) = session::token(&headers) {
        session::close(&pool, token).await.map_err(|error| {
            eprintln!("api: sign-out: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    }
    let cookie = format!("{}; Max-Age=0", session::cookie(""));
    Ok((StatusCode::NO_CONTENT, [(header::SET_COOKIE, cookie)]))
}

async fn statement(
    Account(_): Account,
    State(pool): State<PgPool>,
    State(courses): State<Arc<path::Path>>,
    Path((course, exercise)): Path<(Id, Id)>,
) -> Result<String, StatusCode> {
    let dir = gate::open_exercise(&pool, &courses, &course, &exercise).await?;
    fs::read_to_string(dir.join("statement.md")).map_err(gate::status_of)
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
    Account(author): Account,
    State(pool): State<PgPool>,
    State(courses): State<Arc<path::Path>>,
    Json(submission): Json<Submission>,
) -> Result<(StatusCode, Json<Submitted>), StatusCode> {
    if submission.source.is_empty() || submission.source.len() > MAX_SOURCE {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    gate::open_exercise(&pool, &courses, &submission.course, &submission.exercise).await?;

    let id = enqueue_replacing(&pool, &author, &submission)
        .await
        .map_err(|error| {
            eprintln!("api: enqueue: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok((StatusCode::ACCEPTED, Json(Submitted { id })))
}

// At most one run in progress per student: a newer one replaces the run still waiting (ADR-0008, ADR-0030)
async fn enqueue_replacing(
    pool: &PgPool,
    author: &str,
    submission: &Submission,
) -> sqlx::Result<Uuid> {
    let mut transaction = pool.begin().await?;

    sqlx::query!(
        "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))",
        author
    )
    .execute(&mut *transaction)
    .await?;

    sqlx::query!(
        "UPDATE jobs SET state = 'replaced', finished_at = now() WHERE author = $1 AND state = 'waiting'",
        author
    )
    .execute(&mut *transaction).await?;

    let id = sqlx::query_scalar!(
        r#"
        INSERT INTO jobs (contract, course, exercise, language_pack, source, author)
        VALUES ($1, $2::text, $3::text, $4::text, $5, $6)
        RETURNING id AS "id!"
        "#,
        CONTRACT,
        submission.course.as_str(),
        submission.exercise.as_str(),
        submission.language_pack.as_str(),
        submission.source,
        author
    )
    .fetch_one(&mut *transaction)
    .await?;

    transaction.commit().await?;
    Ok(id)
}

#[derive(Serialize)]
struct Progress {
    state: String,
    passed: Option<bool>,
}

async fn progress(
    Account(author): Account,
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
) -> Result<Json<Progress>, StatusCode> {
    // Someone else's job is not found rather than forbidden: a 403 would confirm it exists
    let progress = sqlx::query_as!(
        Progress,
        r#"
            SELECT state::text AS "state!", (result -> 'cases') <@ '["passed"]' AS "passed?"
            FROM jobs WHERE id = $1 AND author = $2
        "#,
        id,
        author
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
    const SUM: &str = r#"{"course": "log200", "exercise": "sum", "language_pack": "python", "source": "print(0)"}"#;

    async fn sign_in_with(pool: &PgPool, directory: Directory, body: &'static str) -> Response {
        let request = Request::post("/sessions")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body))
            .unwrap();
        app(pool.clone(), directory, PathBuf::new())
            .oneshot(request)
            .await
            .unwrap()
    }

    async fn sign_out_with(pool: &PgPool, cookie: &str) -> Response {
        let request = Request::delete("/sessions")
            .header(header::COOKIE, cookie)
            .body(Body::empty())
            .unwrap();
        app(pool.clone(), Directory::unreachable(), PathBuf::new())
            .oneshot(request)
            .await
            .unwrap()
    }

    async fn signed_in(pool: &PgPool) -> String {
        signed_in_as(pool, "etudiant1").await
    }

    async fn signed_in_as(pool: &PgPool, account: &str) -> String {
        let token = session::open(pool, account).await.unwrap();
        format!("{}={token}", session::COOKIE)
    }

    async fn sessions(pool: &PgPool) -> i64 {
        sqlx::query_scalar("SELECT count(*) FROM sessions")
            .fetch_one(pool)
            .await
            .unwrap()
    }

    async fn post_submission(
        pool: &PgPool,
        courses: PathBuf,
        cookie: &str,
        body: &'static str,
    ) -> StatusCode {
        let request = Request::post("/submissions")
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::COOKIE, cookie)
            .body(Body::from(body))
            .unwrap();
        app(pool.clone(), Directory::unreachable(), courses)
            .oneshot(request)
            .await
            .unwrap()
            .status()
    }

    async fn fetch(router: Router, cookie: &str, uri: &str) -> (StatusCode, String) {
        let request = Request::get(uri)
            .header(header::COOKIE, cookie)
            .body(Body::empty())
            .unwrap();
        let response = router.oneshot(request).await.unwrap();
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, String::from_utf8(body.to_vec()).unwrap())
    }

    async fn get_submission(pool: &PgPool, cookie: &str, id: &str) -> (StatusCode, String) {
        let router = app(pool.clone(), Directory::unreachable(), PathBuf::new());
        fetch(router, cookie, &format!("/submissions/{id}")).await
    }

    // The log200 course with the sum exercise, as the publisher leaves it
    fn published_courses(name: &str) -> PathBuf {
        let courses = env::temp_dir().join("api-tests").join(name);
        let sum = courses.join("log200/releases/abc123/exercises/sum");
        fs::create_dir_all(&sum).unwrap();
        fs::write(sum.join("statement.md"), "# Somme").unwrap();
        fs::write(courses.join("log200/current"), "abc123").unwrap();
        courses
    }

    async fn get_statement(
        pool: &PgPool,
        courses: PathBuf,
        cookie: &str,
        exercise: &str,
    ) -> (StatusCode, String) {
        let router = app(pool.clone(), Directory::unreachable(), courses);
        let uri = format!("/courses/log200/exercises/{exercise}/statement");
        fetch(router, cookie, &uri).await
    }

    async fn schedule_sum(pool: &PgPool, opens_in: &str, closes_in: Option<&str>) {
        sqlx::query(
            "INSERT INTO schedules VALUES ('log200', 'sum', now() + $1::interval, now() + $2::interval)",
        )
        .bind(opens_in)
        .bind(closes_in)
        .execute(pool)
        .await
        .unwrap();
    }

    async fn waiting_jobs(pool: &PgPool) -> i64 {
        sqlx::query_scalar("SELECT count(*) FROM jobs WHERE state = 'waiting'")
            .fetch_one(pool)
            .await
            .unwrap()
    }

    async fn enqueue(pool: &PgPool) -> Uuid {
        sqlx::query_scalar(
            r#"
                INSERT INTO jobs (contract, course, exercise, language_pack, source, author)
                VALUES (1, 'log200', 'sum', 'python', 'print(0)', 'etudiant1')
                RETURNING id
            "#,
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

    async fn open_sum(pool: &PgPool, name: &str) -> PathBuf {
        schedule_sum(pool, "-1 hour", Some("1 hour")).await;
        published_courses(name)
    }

    #[tokio::test]
    async fn health_is_ok() {
        let pool = PgPool::connect_lazy("postgres://unused").unwrap();
        let request = Request::get("/health").body(Body::empty()).unwrap();
        let response = app(pool, Directory::unreachable(), PathBuf::new())
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

    #[tokio::test]
    async fn sign_out_without_session_still_clears_the_cookie() {
        let pool = PgPool::connect_lazy("postgres://unused").unwrap();
        let response = sign_out_with(&pool, "").await;
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert_eq!(
            response.headers()[header::SET_COOKIE],
            "__Host-session=; Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age=0"
        );
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn open_exercise_shows_its_statement(pool: PgPool) {
        schedule_sum(&pool, "-1 hour", Some("1 hour")).await;
        let courses = published_courses("statement-open");
        assert_eq!(
            get_statement(&pool, courses, &signed_in(&pool).await, "sum").await,
            (StatusCode::OK, "# Somme".to_string())
        );
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn exercise_without_closing_stays_open(pool: PgPool) {
        schedule_sum(&pool, "-1 hour", None).await;
        let courses = published_courses("statement-practice");
        assert_eq!(
            get_statement(&pool, courses, &signed_in(&pool).await, "sum")
                .await
                .0,
            StatusCode::OK
        );
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn exercise_not_yet_open_is_forbidden(pool: PgPool) {
        schedule_sum(&pool, "1 hour", Some("2 hours")).await;
        let courses = published_courses("statement-not-yet-open");
        assert_eq!(
            get_statement(&pool, courses, &signed_in(&pool).await, "sum")
                .await
                .0,
            StatusCode::FORBIDDEN
        );
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn exercise_without_schedule_is_forbidden(pool: PgPool) {
        let courses = published_courses("statement-unscheduled");
        assert_eq!(
            get_statement(&pool, courses, &signed_in(&pool).await, "sum")
                .await
                .0,
            StatusCode::FORBIDDEN
        );
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn unknown_exercise_is_not_found(pool: PgPool) {
        let courses = published_courses("statement-unknown");
        assert_eq!(
            get_statement(&pool, courses, &signed_in(&pool).await, "missing")
                .await
                .0,
            StatusCode::NOT_FOUND
        );
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn path_outside_the_course_is_a_bad_request(pool: PgPool) {
        let courses = published_courses("statement-escape");
        assert_eq!(
            get_statement(&pool, courses, &signed_in(&pool).await, "%2E%2E")
                .await
                .0,
            StatusCode::BAD_REQUEST
        );
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn statement_without_session_is_unauthorized(pool: PgPool) {
        schedule_sum(&pool, "-1 hour", Some("1 hour")).await;
        let courses = published_courses("statement-anonymous");
        assert_eq!(
            get_statement(&pool, courses, "", "sum").await.0,
            StatusCode::UNAUTHORIZED
        );
    }

    #[sqlx::test(migrations = false)]
    #[ignore = "needs PostgreSQL"]
    async fn migrations_apply_once(pool: PgPool) {
        MIGRATOR.run(&pool).await.unwrap();
        MIGRATOR.run(&pool).await.unwrap();
        assert_eq!(waiting_jobs(&pool).await, 0);
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn submission_is_queued(pool: PgPool) {
        let body = r#"{"course": "log200", "exercise": "sum", "language_pack": "python", "source": "print(0)"}"#;
        assert_eq!(
            post_submission(
                &pool,
                open_sum(&pool, "submission-queued").await,
                &signed_in(&pool).await,
                body
            )
            .await,
            StatusCode::ACCEPTED
        );
        assert_eq!(waiting_jobs(&pool).await, 1);
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn invalid_identifier_is_rejected(pool: PgPool) {
        let body = r#"{"course": "log200", "exercise": "../sum", "language_pack": "python", "source": "print(0)"}"#;
        assert_eq!(
            post_submission(&pool, PathBuf::new(), &signed_in(&pool).await, body).await,
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
            post_submission(&pool, PathBuf::new(), &signed_in(&pool).await, body).await,
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
    async fn submission_records_its_author(pool: PgPool) {
        let body = r#"{"course": "log200", "exercise": "sum", "language_pack": "python", "source": "print(0)"}"#;
        let cookie = signed_in_as(&pool, "etudiant2").await;
        assert_eq!(
            post_submission(
                &pool,
                open_sum(&pool, "submission-author").await,
                &cookie,
                body
            )
            .await,
            StatusCode::ACCEPTED
        );
        let author: String = sqlx::query_scalar("SELECT author FROM jobs")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(author, "etudiant2");
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn someone_elses_submission_is_not_found(pool: PgPool) {
        let job = enqueue(&pool).await;
        let cookie = signed_in_as(&pool, "etudiant2").await;
        assert_eq!(
            get_submission(&pool, &cookie, &job.to_string()).await,
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
            post_submission(&pool, PathBuf::new(), "", body).await,
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
            post_submission(&pool, PathBuf::new(), &cookie, body).await,
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
            post_submission(
                &pool,
                open_sum(&pool, "submission-cookie").await,
                &cookie,
                body
            )
            .await,
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

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn submission_to_a_closed_exercise_is_forbidden(pool: PgPool) {
        schedule_sum(&pool, "-2 hours", Some("-1 hour")).await;
        let courses = published_courses("submission-closed");
        let body = r#"{"course": "log200", "exercise": "sum", "language_pack": "python", "source": "print(0)"}"#;
        assert_eq!(
            post_submission(&pool, courses, &signed_in(&pool).await, body).await,
            StatusCode::FORBIDDEN
        );
        assert_eq!(waiting_jobs(&pool).await, 0);
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn submission_to_an_unpublished_exercise_is_not_found(pool: PgPool) {
        let courses = published_courses("submission-unpublished");
        let body = r#"{"course": "log200", "exercise": "missing", "language_pack": "python", "source": "print(0)"}"#;
        assert_eq!(
            post_submission(&pool, courses, &signed_in(&pool).await, body).await,
            StatusCode::NOT_FOUND
        );
        assert_eq!(waiting_jobs(&pool).await, 0);
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn sign_out_closes_only_its_session(pool: PgPool) {
        let leaving = signed_in(&pool).await;
        let staying = signed_in_as(&pool, "etudiant2").await;
        assert_eq!(
            sign_out_with(&pool, &leaving).await.status(),
            StatusCode::NO_CONTENT
        );
        assert_eq!(sessions(&pool).await, 1);

        let nil = Uuid::nil().to_string();
        assert_eq!(
            get_submission(&pool, &leaving, &nil).await.0,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            get_submission(&pool, &staying, &nil).await.0,
            StatusCode::NOT_FOUND
        );
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn signing_out_twice_is_no_content(pool: PgPool) {
        let cookie = signed_in(&pool).await;
        sign_out_with(&pool, &cookie).await;
        assert_eq!(
            sign_out_with(&pool, &cookie).await.status(),
            StatusCode::NO_CONTENT
        );
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn new_submission_replaces_the_waiting_one(pool: PgPool) {
        let previous = enqueue(&pool).await;
        let cookie = signed_in(&pool).await;
        let courses = open_sum(&pool, "submission-replaces").await;
        assert_eq!(
            post_submission(&pool, courses, &cookie, SUM).await,
            StatusCode::ACCEPTED
        );
        assert_eq!(
            get_submission(&pool, &cookie, &previous.to_string()).await,
            (
                StatusCode::OK,
                r#"{"state":"replaced","passed":null}"#.to_string()
            )
        );
        assert_eq!(waiting_jobs(&pool).await, 1);
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn running_submission_is_not_replaced(pool: PgPool) {
        let previous = enqueue(&pool).await;
        sqlx::query("SELECT id FROM claim_job()")
            .execute(&pool)
            .await
            .unwrap();
        let cookie = signed_in(&pool).await;
        let courses = open_sum(&pool, "submission-running").await;
        assert_eq!(
            post_submission(&pool, courses, &cookie, SUM).await,
            StatusCode::ACCEPTED
        );
        assert_eq!(
            get_submission(&pool, &cookie, &previous.to_string()).await,
            (
                StatusCode::OK,
                r#"{"state":"running","passed":null}"#.to_string()
            )
        );
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn other_students_waiting_submission_stays(pool: PgPool) {
        enqueue(&pool).await;
        let cookie = signed_in_as(&pool, "etudiant2").await;
        let courses = open_sum(&pool, "submission-other-student").await;
        assert_eq!(
            post_submission(&pool, courses, &cookie, SUM).await,
            StatusCode::ACCEPTED
        );
        assert_eq!(waiting_jobs(&pool).await, 2);
    }
}
