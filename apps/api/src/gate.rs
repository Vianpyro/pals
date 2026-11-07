//! The API's side of the single gate: an exercise is read of submitted only while open (ADR-0020)

use std::{
    io,
    path::{Path, PathBuf},
    time::SystemTime,
};

use axum::http::StatusCode;
use content::accessible_exercise;
use contracts::Id;
use sqlx::PgPool;

use super::{roles, schedule};

pub(super) async fn member(pool: &PgPool, course: &Id, account: &str) -> Result<(), StatusCode> {
    let has_role = roles::has_role(pool, course, account)
        .await
        .map_err(|error| {
            eprintln!("api: roles: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    has_role.ok_or(StatusCode::FORBIDDEN)
}

pub(super) async fn open_exercise(
    pool: &PgPool,
    courses: &Path,
    account: &str,
    course: &Id,
    exercise: &Id,
) -> Result<PathBuf, StatusCode> {
    member(pool, course, account).await?;

    let schedule = schedule::read(pool, course, exercise)
        .await
        .map_err(|error| {
            eprintln!("api: schedule: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let course_dir = courses.join(course.as_str());
    accessible_exercise(&course_dir, exercise, &schedule, SystemTime::now()).map_err(status_of)
}

pub(super) fn status_of(error: io::Error) -> StatusCode {
    match error.kind() {
        io::ErrorKind::NotFound => StatusCode::NOT_FOUND,
        io::ErrorKind::PermissionDenied => StatusCode::FORBIDDEN,
        _ => {
            eprintln!("api: exercise: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}
