//! Active release reader and the single opening rule.

mod catalog;
mod contest;
mod exercise;
mod gate;
mod grade;
mod release;
mod schedule;

pub use catalog::exercises;
pub use contest::{Contest, Slot, contest};
pub use gate::{Seat, accessible_exercise, contest_exercise};
pub use grade::{Final, contest_grade};
pub use release::active_release;
pub use schedule::{Accommodation, ExtraTime, Schedule, Window, session_closes_at};

#[cfg(test)]
mod fixtures;
