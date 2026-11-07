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
use contracts::{Credentials, Id, Identity, Submission, Submitted};
use directory::{Directory, SignInError};
use kind::JobKind;
use notify::RunChanged;
use session::Account;
use sqlx::{PgPool, migrate::Migrator};
use tokio::sync::broadcast;
use uuid::Uuid;

mod catalog;
mod directory;
mod drafts;
mod events;
mod gate;
mod kind;
mod notify;
mod runs;
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
            let runs = notify::channel();
            tokio::spawn(notify::forward(pool.clone(), runs.clone()));
            axum::serve(listener, app(pool, directory, courses.into(), runs)).await?;
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
    runs: broadcast::Sender<RunChanged>,
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

impl FromRef<AppState> for broadcast::Sender<RunChanged> {
    fn from_ref(state: &AppState) -> Self {
        state.runs.clone()
    }
}

fn app(
    pool: PgPool,
    directory: Directory,
    courses: PathBuf,
    runs: broadcast::Sender<RunChanged>,
) -> Router {
    let state = AppState {
        pool,
        directory: Arc::new(directory),
        courses: Arc::from(courses),
        runs,
    };
    Router::new()
        .route("/courses/{course}/exercises", get(catalog::catalog))
        .route(
            "/courses/{course}/exercises/{exercise}/draft",
            get(drafts::draft).put(drafts::save),
        )
        .route(
            "/courses/{course}/exercises/{exercise}/statement",
            get(statement),
        )
        .route(
            "/courses/{course}/exercises/{exercise}/submissions",
            get(runs::history),
        )
        .route("/events", get(events::events))
        .route("/health", get(|| async { StatusCode::OK }))
        .route("/sessions", post(sign_in).delete(sign_out))
        .route("/submissions", post(submit))
        .route("/submissions/{id}", get(runs::progress))
        .with_state(state)
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
        INSERT INTO jobs (contract, course, exercise, language_pack, source, author, kind)
        VALUES ($1, $2::text, $3::text, $4::text, $5, $6, $7)
        RETURNING id AS "id!"
        "#,
        CONTRACT,
        submission.course.as_str(),
        submission.exercise.as_str(),
        submission.language_pack.as_str(),
        submission.source,
        author,
        JobKind::from(submission.kind) as JobKind,
    )
    .fetch_one(&mut *transaction)
    .await?;

    transaction.commit().await?;
    Ok(id)
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

    // A router without the queue's notifications. Tests build one per request, and each listener
    // would hold one of the test pool's 5 connections until the test ends (sqlx::test)
    fn app(pool: PgPool, directory: Directory, courses: PathBuf) -> Router {
        super::app(pool, directory, courses, notify::channel())
    }

    struct Events {
        body: axum::body::BodyDataStream,
        pending: String,
    }

    impl Events {
        async fn open(pool: &PgPool, cookie: &str) -> (StatusCode, Events) {
            let request = Request::get("/events")
                .header(header::COOKIE, cookie)
                .body(Body::empty())
                .unwrap();
            let runs = notify::channel();
            tokio::spawn(notify::forward(pool.clone(), runs.clone()));
            let response = super::app(pool.clone(), Directory::unreachable(), PathBuf::new(), runs)
                .oneshot(request)
                .await
                .unwrap();
            let status = response.status();
            let body = response.into_body().into_data_stream();
            (
                status,
                Events {
                    body,
                    pending: String::new(),
                },
            )
        }

        async fn next(&mut self) -> Option<String> {
            use futures_util::StreamExt;
            loop {
                if let Some(end) = self.pending.find("\n\n") {
                    let event: String = self.pending.drain(..end + 2).collect();
                    match event.lines().find_map(|line| line.strip_prefix("data: ")) {
                        Some(data) => return Some(data.to_string()),
                        None => continue, // a keep-alive comment
                    }
                }
                let chunk =
                    tokio::time::timeout(std::time::Duration::from_secs(2), self.body.next())
                        .await
                        .ok()??
                        .unwrap();
                self.pending.push_str(std::str::from_utf8(&chunk).unwrap());
            }
        }
    }

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
        read(router.oneshot(request).await.unwrap()).await
    }

    async fn read(response: Response) -> (StatusCode, String) {
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

    async fn waiting_kind(pool: &PgPool) -> String {
        sqlx::query_scalar("SELECT kind::text FROM jobs WHERE state = 'waiting'")
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

    async fn enqueue_at(pool: &PgPool, author: &str, exercise: &str, at: &str) -> Uuid {
        sqlx::query_scalar(
            r#"
                INSERT INTO jobs (contract, course, exercise, language_pack, source, author, enqueued_at)
                VALUES (1, 'log200', $1, 'python', 'print(0)', $2, $3::timestamptz)
                RETURNING id
            "#,
        )
        .bind(exercise)
        .bind(author)
        .bind(at)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    async fn get_history(pool: &PgPool, cookie: &str) -> (StatusCode, String) {
        let router = app(pool.clone(), Directory::unreachable(), PathBuf::new());
        fetch(router, cookie, "/courses/log200/exercises/sum/submissions").await
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

    async fn listening(pool: &PgPool) {
        for _ in 0..100 {
            let listeners: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM pg_stat_activity WHERE datname = current_database() AND query = 'LISTEN \"runs\"'",
            )
            .fetch_one(pool)
            .await
            .unwrap();
            if listeners > 0 {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        panic!("the API never listened");
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
    async fn precheck_is_queued_as_a_precheck(pool: PgPool) {
        let body = r#"{"course": "log200", "exercise": "sum", "language_pack": "python", "source": "print(0)", "kind": "precheck"}"#;
        let courses = open_sum(&pool, "submission-precheck").await;
        let cookie = signed_in(&pool).await;
        assert_eq!(
            post_submission(&pool, courses, &cookie, body).await,
            StatusCode::ACCEPTED
        );
        assert_eq!(waiting_kind(&pool).await, "precheck");
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn submission_without_a_kind_is_queued_as_a_check(pool: PgPool) {
        let courses = open_sum(&pool, "submission-no-kind").await;
        let cookie = signed_in(&pool).await;
        assert_eq!(
            post_submission(&pool, courses, &cookie, SUM).await,
            StatusCode::ACCEPTED
        );
        assert_eq!(waiting_kind(&pool).await, "check");
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn final_submission_cannot_be_requested(pool: PgPool) {
        let body = r#"{"course": "log200", "exercise": "sum", "language_pack": "python", "source": "print(0)", "kind": "final"}"#;
        let courses = open_sum(&pool, "submission-final").await;
        let cookie = signed_in(&pool).await;
        assert_eq!(
            post_submission(&pool, courses, &cookie, body).await,
            StatusCode::UNPROCESSABLE_ENTITY
        );
        assert_eq!(waiting_jobs(&pool).await, 0);
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
            (StatusCode::OK, r#"{"state":"waiting"}"#.to_string())
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
            (StatusCode::OK, r#"{"state":"replaced"}"#.to_string())
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
            (StatusCode::OK, r#"{"state":"running"}"#.to_string())
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

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn catalog_lists_open_and_locked_exercises(pool: PgPool) {
        sqlx::query("INSERT INTO schedules VALUES ('log200', 'sum', '2000-01-01T00:00:00Z', NULL)")
            .execute(&pool)
            .await
            .unwrap();
        let courses = published_courses("catalog");
        fs::create_dir_all(courses.join("log200/releases/abc123/exercises/product")).unwrap();

        let router = app(pool.clone(), Directory::unreachable(), courses);
        let cookie = signed_in(&pool).await;
        assert_eq!(
            fetch(router, &cookie, "/courses/log200/exercises").await,
            (
                StatusCode::OK,
                concat!(
                    r#"[{"id":"product","open":false,"opens_at":null,"closes_at":null},"#,
                    r#"{"id":"sum","open":true,"opens_at":"2000-01-01T00:00:00Z","closes_at":null}]"#
                )
                .to_string()
            )
        );
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn catalog_without_session_is_unauthorized(pool: PgPool) {
        let router = app(
            pool,
            Directory::unreachable(),
            published_courses("catalog-anonymous"),
        );
        assert_eq!(
            fetch(router, "", "/courses/log200/exercises").await.0,
            StatusCode::UNAUTHORIZED
        );
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn catalog_of_an_unknown_course_is_not_found(pool: PgPool) {
        let router = app(
            pool.clone(),
            Directory::unreachable(),
            published_courses("catalog-unknown"),
        );
        assert_eq!(
            fetch(router, &signed_in(&pool).await, "/courses/log121/exercises")
                .await
                .0,
            StatusCode::NOT_FOUND
        );
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn history_lists_own_runs_newest_first(pool: PgPool) {
        let older = enqueue_at(&pool, "etudiant1", "sum", "2026-01-01T08:00:00Z").await;
        judge(&pool, older, "{passed,wrong-answer}").await;
        let newer = enqueue_at(&pool, "etudiant1", "sum", "2026-01-01T09:00:00Z").await;

        assert_eq!(
            get_history(&pool, &signed_in(&pool).await).await,
            (
                StatusCode::OK,
                format!(
                    concat!(
                        r#"[{{"id":"{newer}","state":"waiting","enqueued_at":"2026-01-01T09:00:00Z"}},"#,
                        r#"{{"id":"{older}","state":"done","passed":false,"enqueued_at":"2026-01-01T08:00:00Z"}}]"#
                    ),
                    newer = newer,
                    older = older
                )
            )
        );
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn history_leaves_out_other_students_and_exercises(pool: PgPool) {
        enqueue_at(&pool, "etudiant2", "sum", "2026-01-01T08:00:00Z").await;
        enqueue_at(&pool, "etudiant1", "product", "2026-01-01T08:00:00Z").await;
        assert_eq!(
            get_history(&pool, &signed_in(&pool).await).await,
            (StatusCode::OK, "[]".to_string())
        );
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn history_without_session_is_unauthorized(pool: PgPool) {
        assert_eq!(get_history(&pool, "").await.0, StatusCode::UNAUTHORIZED);
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn events_start_with_the_latest_run_of_each_exercise(pool: PgPool) {
        enqueue_at(&pool, "etudiant1", "sum", "2026-01-01T08:00:00Z").await;
        let newer = enqueue_at(&pool, "etudiant1", "sum", "2026-01-01T09:00:00Z").await;
        let product = enqueue_at(&pool, "etudiant1", "product", "2026-01-01T08:00:00Z").await;
        enqueue_at(&pool, "etudiant2", "sum", "2026-01-01T10:00:00Z").await;

        let (status, mut events) = Events::open(&pool, &signed_in(&pool).await).await;
        assert_eq!(status, StatusCode::OK);
        // By course, then exercise: product before sum
        assert_eq!(
            events.next().await,
            Some(format!(
                r#"{{"id":"{product}","state":"waiting","enqueued_at":"2026-01-01T08:00:00Z"}}"#
            ))
        );
        assert_eq!(
            events.next().await,
            Some(format!(
                r#"{{"id":"{newer}","state":"waiting","enqueued_at":"2026-01-01T09:00:00Z"}}"#
            ))
        );
        assert_eq!(events.next().await, None);
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn events_follow_ones_runs_and_no_one_elses(pool: PgPool) {
        let (_, mut events) = Events::open(&pool, &signed_in(&pool).await).await;
        listening(&pool).await;

        let mine = enqueue(&pool).await;
        let next = events.next().await.unwrap();
        assert!(
            next.contains(&format!(r#""id":"{mine}","state":"waiting""#)),
            "{next}"
        );

        judge(&pool, mine, "{passed}").await;
        let mut next = events.next().await.unwrap();
        while next.contains(r#""state":"running""#) {
            next = events.next().await.unwrap();
        }
        assert!(
            next.contains(&format!(r#""id":"{mine}","state":"done","passed":true"#)),
            "{next}"
        );

        enqueue_at(&pool, "etudiant2", "sum", "2026-01-01T08:00:00Z").await;
        let second = enqueue_at(&pool, "etudiant1", "product", "2026-01-01T08:00:00Z").await;
        let next = loop {
            let next = events.next().await.unwrap();
            if !next.contains(&mine.to_string()) {
                break next;
            }
        };
        assert!(next.contains(&second.to_string()), "{next}");
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn events_without_session_are_unauthorized(pool: PgPool) {
        assert_eq!(Events::open(&pool, "").await.0, StatusCode::UNAUTHORIZED);
    }

    async fn put_draft(
        pool: &PgPool,
        courses: PathBuf,
        cookie: &str,
        body: impl Into<Body>,
    ) -> (StatusCode, String) {
        let request = Request::put("/courses/log200/exercises/sum/draft")
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::COOKIE, cookie)
            .body(body.into())
            .unwrap();
        let router = app(pool.clone(), Directory::unreachable(), courses);
        read(router.oneshot(request).await.unwrap()).await
    }

    async fn get_draft(pool: &PgPool, courses: PathBuf, cookie: &str) -> (StatusCode, String) {
        let router = app(pool.clone(), Directory::unreachable(), courses);
        fetch(router, cookie, "/courses/log200/exercises/sum/draft").await
    }

    fn answer(status: StatusCode, body: &str) -> (StatusCode, String) {
        (status, body.to_string())
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn no_draft_yet_is_revision_zero(pool: PgPool) {
        let courses = open_sum(&pool, "draft-none").await;
        assert_eq!(
            get_draft(&pool, courses, &signed_in(&pool).await).await,
            answer(StatusCode::OK, r#"{"revision":0,"source":""}"#)
        );
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn a_saved_draft_is_read_back(pool: PgPool) {
        let courses = open_sum(&pool, "draft-saved").await;
        let cookie = signed_in(&pool).await;
        let first = r#"{"base":0,"source":"print(1)"}"#;
        let second = r#"{"base":1,"source":"print(2)"}"#;
        assert_eq!(
            put_draft(&pool, courses.clone(), &cookie, first).await,
            answer(StatusCode::OK, r#"{"revision":1}"#)
        );
        assert_eq!(
            put_draft(&pool, courses.clone(), &cookie, second).await,
            answer(StatusCode::OK, r#"{"revision":2}"#)
        );
        assert_eq!(
            get_draft(&pool, courses, &cookie).await,
            answer(StatusCode::OK, r#"{"revision":2,"source":"print(2)"}"#)
        );
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn a_stale_write_gets_the_current_draft(pool: PgPool) {
        let courses = open_sum(&pool, "draft-stale").await;
        let cookie = signed_in(&pool).await;
        put_draft(
            &pool,
            courses.clone(),
            &cookie,
            r#"{"base":0,"source":"print(1)"}"#,
        )
        .await;
        assert_eq!(
            put_draft(
                &pool,
                courses.clone(),
                &cookie,
                r#"{"base":0,"source":"print(2)"}"#
            )
            .await,
            answer(
                StatusCode::CONFLICT,
                r#"{"revision":1,"source":"print(1)"}"#
            )
        );
        assert_eq!(
            get_draft(&pool, courses, &cookie).await,
            answer(StatusCode::OK, r#"{"revision":1,"source":"print(1)"}"#)
        );
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn a_replayed_write_is_acknowledged_again(pool: PgPool) {
        let courses = open_sum(&pool, "draft-replayed").await;
        let cookie = signed_in(&pool).await;
        let write = r#"{"base":0,"source":"print(1)"}"#;
        for _ in 0..2 {
            assert_eq!(
                put_draft(&pool, courses.clone(), &cookie, write).await,
                answer(StatusCode::OK, r#"{"revision":1}"#)
            );
        }
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn of_two_writes_on_one_base_only_one_applies(pool: PgPool) {
        let courses = open_sum(&pool, "draft-race").await;
        let cookie = signed_in(&pool).await;
        let (first, second) = tokio::join!(
            put_draft(
                &pool,
                courses.clone(),
                &cookie,
                r#"{"base":0,"source":"print(1)"}"#
            ),
            put_draft(
                &pool,
                courses.clone(),
                &cookie,
                r#"{"base":0,"source":"print(2)"}"#
            ),
        );
        let mut statuses = [first.0, second.0];
        statuses.sort();
        assert_eq!(statuses, [StatusCode::OK, StatusCode::CONFLICT]);
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn drafts_are_per_student(pool: PgPool) {
        let courses = open_sum(&pool, "draft-own").await;
        let mine = signed_in_as(&pool, "etudiant1").await;
        put_draft(
            &pool,
            courses.clone(),
            &mine,
            r#"{"base":0,"source":"print(1)"}"#,
        )
        .await;
        assert_eq!(
            get_draft(&pool, courses, &signed_in_as(&pool, "etudiant2").await).await,
            answer(StatusCode::OK, r#"{"revision":0,"source":""}"#)
        );
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn a_draft_of_a_closed_exercise_is_forbidden(pool: PgPool) {
        schedule_sum(&pool, "-2 hours", Some("-1 hour")).await;
        let courses = published_courses("draft-closed");
        let cookie = signed_in(&pool).await;
        let write = r#"{"base":0,"source":"print(1)"}"#;
        assert_eq!(
            put_draft(&pool, courses.clone(), &cookie, write).await.0,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            get_draft(&pool, courses, &cookie).await.0,
            StatusCode::FORBIDDEN
        );
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn an_oversized_draft_is_rejected(pool: PgPool) {
        let courses = open_sum(&pool, "draft-oversized").await;
        let write = format!(r#"{{"base":0,"source":"{}"}}"#, "x".repeat(MAX_SOURCE + 1));
        assert_eq!(
            put_draft(&pool, courses, &signed_in(&pool).await, write)
                .await
                .0,
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }
}
