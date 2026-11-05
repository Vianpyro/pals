//! A program sent to be judged, and the run the queue made of it

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::Id;

#[derive(Deserialize, TS)]
#[ts(export)]
pub struct Submission {
    pub course: Id,
    pub exercise: Id,
    pub language_pack: Id,
    pub source: String,
}

#[derive(Serialize, TS)]
#[ts(export)]
pub struct Submitted {
    pub id: Uuid,
}
