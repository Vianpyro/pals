//! Active release reader and the single opening rule.

mod catalog;
mod exercise;
mod gate;
mod release;
mod schedule;

pub use catalog::exercises;
pub use gate::accessible_exercise;
pub use release::active_release;
pub use schedule::{Accommodation, ExtraTime, Schedule, Window, session_closes_at};

#[cfg(test)]
mod fixtures;
