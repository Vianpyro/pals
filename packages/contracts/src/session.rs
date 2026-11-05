//! Signing in: what the student types, and who the directory says they are (ADR-0017)

use serde::{Deserialize, Serialize};
use ts_rs::TS;

// No Debug: it holds the password
#[derive(Deserialize, TS)]
#[ts(export)]
pub struct Credentials {
    pub id: String,
    pub password: String,
}

#[derive(Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct Identity {
    pub id: String,
    pub name: Option<String>,
    pub email: Option<String>,
}
