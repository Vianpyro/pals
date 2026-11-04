//! A course's catalog: every published exercise with its dates, open or not (ADR-0020)

use std::{path, sync::Arc, time::SystemTime};

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use contracts::Id;
use serde::Serialize;
use sqlx::PgPool;
use time::OffsetDateTime;

use super::{gate, schedule, session::Account};

#[derive(Serialize)]
pub(super) struct Entry {
    id: Id,
    open: bool,
    #[serde(with = "time::serde::rfc3339::option")]
    opens_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    closes_at: Option<OffsetDateTime>,
}

pub(super) async fn catalog(
    Account(_): Account,
    State(pool): State<PgPool>,
    State(courses): State<Arc<path::Path>>,
    Path(course): Path<Id>,
) -> Result<Json<Vec<Entry>>, StatusCode> {
    let ids = content::exercises(&courses.join(course.as_str())).map_err(gate::status_of)?;
    // One instant for the whole list: two exercises never disagree about what time it is
    let now = SystemTime::now();

    let mut entries = Vec::with_capacity(ids.len());
    for id in ids {
        let schedule = schedule::read(&pool, &course, &id).await.map_err(|error| {
            eprintln!("api: catalog: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        let window = schedule.window();
        entries.push(Entry {
            id,
            open: schedule.is_open(now),
            opens_at: window.map(|window| window.opens_at.into()),
            closes_at: window.and_then(|window| window.closes_at).map(Into::into),
        });
    }

    Ok(Json(entries))
}
