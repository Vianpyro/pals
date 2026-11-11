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

pub(super) async fn course_dir(
    pool: &PgPool,
    courses: &Path,
    offering: &Id,
    account: &str,
) -> Result<PathBuf, StatusCode> {
    let course = roles::course(pool, offering, account)
        .await
        .map_err(|error| {
            eprintln!("api: roles: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    // Safe in a path: the identifier domain allows only [a-z0-9-] (ADR-0023)
    course
        .map(|course| courses.join(course))
        .ok_or(StatusCode::FORBIDDEN)
}

pub(super) async fn open_exercise(
    pool: &PgPool,
    courses: &Path,
    account: &str,
    offering: &Id,
    exercise: &Id,
) -> Result<PathBuf, StatusCode> {
    let course_dir = course_dir(pool, courses, offering, account).await?;

    let schedule = schedule::read(pool, offering, exercise)
        .await
        .map_err(|error| {
            eprintln!("api: schedule: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

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
