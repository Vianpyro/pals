//! A student's draft of an exercise. The server's copy is the truth (ADR-0038)

use std::{path, sync::Arc};

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use contracts::{Draft, DraftSaved, DraftWrite, Id};
use sqlx::PgPool;

use super::{MAX_SOURCE, gate, session::Account};

fn internal_error(error: impl std::fmt::Display) -> StatusCode {
    eprintln!("api: draft: {error}");
    StatusCode::INTERNAL_SERVER_ERROR
}

fn revision(stored: i64) -> Result<u32, StatusCode> {
    u32::try_from(stored).map_err(internal_error)
}

fn replayed(write: &DraftWrite, current: &Draft) -> bool {
    write.base.checked_add(1) == Some(current.revision) && write.source == current.source
}

async fn current(
    pool: &PgPool,
    author: &str,
    course: &Id,
    exercise: &Id,
) -> Result<Draft, StatusCode> {
    let Some(row) = sqlx::query!(
        "SELECT revision, source FROM drafts WHERE author = $1 AND course = $2::text AND exercise = $3::text",
        author,
        course.as_str(),
        exercise.as_str(),
    )
    .fetch_optional(pool)
    .await
    .map_err(internal_error)?
    else {
        return Ok(Draft::default());
    };

    Ok(Draft {
        revision: revision(row.revision)?,
        source: row.source,
    })
}

async fn apply(
    pool: &PgPool,
    author: &str,
    course: &Id,
    exercise: &Id,
    write: &DraftWrite,
) -> Result<Option<u32>, StatusCode> {
    sqlx::query_scalar!(
        r#"
            INSERT INTO drafts (author, course, exercise, revision, source, language_pack)
            VALUES ($1, $2::text, $3::text, $4::bigint + 1, $5, $6::text)
            ON CONFLICT (author, course, exercise) DO UPDATE
                SET revision = excluded.revision, source = excluded.source,
                    language_pack = excluded.language_pack, saved_at = now()
                WHERE drafts.revision = $4::bigint
            RETURNING revision
        "#,
        author,
        course.as_str(),
        exercise.as_str(),
        i64::from(write.base),
        write.source,
        write.language_pack.as_str()
    )
    .fetch_optional(pool)
    .await
    .map_err(internal_error)?
    .map(revision)
    .transpose()
}

pub(super) async fn draft(
    Account(author): Account,
    State(pool): State<PgPool>,
    State(courses): State<Arc<path::Path>>,
    Path((course, exercise)): Path<(Id, Id)>,
) -> Result<Json<Draft>, StatusCode> {
    gate::open_exercise(&pool, &courses, &author, &course, &exercise).await?;
    current(&pool, &author, &course, &exercise).await.map(Json)
}

pub(super) async fn save(
    Account(author): Account,
    State(pool): State<PgPool>,
    State(courses): State<Arc<path::Path>>,
    Path((course, exercise)): Path<(Id, Id)>,
    Json(write): Json<DraftWrite>,
) -> Result<Response, StatusCode> {
    if write.source.len() > MAX_SOURCE {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    gate::open_exercise(&pool, &courses, &author, &course, &exercise).await?;

    if let Some(revision) = apply(&pool, &author, &course, &exercise, &write).await? {
        return Ok(Json(DraftSaved { revision }).into_response());
    }

    let current = current(&pool, &author, &course, &exercise).await?;
    if replayed(&write, &current) {
        return Ok(Json(DraftSaved {
            revision: current.revision,
        })
        .into_response());
    }

    // Not an error: the student chooses between their version and this one (ADR-0038)
    Ok((StatusCode::CONFLICT, Json(current)).into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(base: u32, source: &str) -> DraftWrite {
        DraftWrite {
            base,
            source: source.to_string(),
            language_pack: "python".parse().unwrap(),
        }
    }

    fn draft(revision: u32, source: &str) -> Draft {
        Draft {
            revision,
            source: source.to_string(),
        }
    }

    #[test]
    fn same_write_is_a_replay() {
        assert!(replayed(&write(3, "print(1)"), &draft(4, "print(1)")));
    }

    #[test]
    fn different_write_is_not_a_replay() {
        assert!(!replayed(&write(3, "print(1)"), &draft(4, "print(2)")));
        assert!(!replayed(&write(2, "print(1)"), &draft(4, "print(1)")));
    }

    #[test]
    fn last_revision_has_no_next() {
        assert!(!replayed(&write(u32::MAX, ""), &draft(0, "")));
    }
}
