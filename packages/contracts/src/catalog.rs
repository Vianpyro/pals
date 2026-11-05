//! A course's catalog: every published exercise with its dates, open or not (ADR-0020)

use serde::Serialize;
use time::OffsetDateTime;
use ts_rs::TS;

use crate::Id;

#[derive(Serialize, TS)]
#[ts(export)]
pub struct CatalogEntry {
    pub id: Id,
    pub open: bool,
    #[serde(with = "time::serde::rfc3339::option")]
    #[ts(type = "string | null")]
    pub opens_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    #[ts(type = "string | null")]
    pub closes_at: Option<OffsetDateTime>,
}
