//! A student's draft of an exercise, saved revision after revision (ADR-0038)

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Default, Serialize, TS)]
#[ts(export)]
pub struct Draft {
    pub revision: u32,
    pub source: String,
}

#[derive(Deserialize, TS)]
#[ts(export)]
pub struct DraftWrite {
    pub base: u32,
    pub source: String,
}

#[derive(Serialize, TS)]
#[ts(export)]
pub struct DraftSaved {
    pub revision: u32,
}
