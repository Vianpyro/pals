//! An offering's catalog: every published exercise with its dates, open or not (ADR-0020)

use std::{path, sync::Arc, time::SystemTime};

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use contracts::{CatalogEntry, Id};
use sqlx::PgPool;

use super::{gate, schedule, session::Account};

pub(super) async fn catalog(
    Account(account): Account,
    State(pool): State<PgPool>,
    State(courses): State<Arc<path::Path>>,
    Path(offering): Path<Id>,
) -> Result<Json<Vec<CatalogEntry>>, StatusCode> {
    let course_dir = gate::course_dir(&pool, &courses, &offering, &account).await?;
    let ids = content::exercises(&course_dir).map_err(gate::status_of)?;
    // One instant for the whole list: two exercises never disagree about what time it is
    let now = SystemTime::now();

    let mut entries = Vec::with_capacity(ids.len());
    for id in ids {
        let schedule = schedule::read(&pool, &offering, &id)
            .await
            .map_err(|error| {
                eprintln!("api: catalog: {error}");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        let window = schedule.window();
        entries.push(CatalogEntry {
            id,
            open: schedule.is_open(now),
            opens_at: window.map(|window| window.opens_at.into()),
            closes_at: window.and_then(|window| window.closes_at).map(Into::into),
        });
    }

    Ok(Json(entries))
}
