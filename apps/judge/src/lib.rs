//! Judging of a submission in a gVisor sandbox (ADR-0002, ADR-0018)

mod cases;
mod choice;
mod errors;
mod judging;
mod kind;
mod pack;
mod queue;
mod report;
mod sandbox;
mod schedule;
mod stdio;
mod submission;
mod verdict;

pub use cases::{Case, Visibility, read_cases};
pub use choice::{AnswerError, ChoiceKey, Selection, grade, read_choice_key};
pub use judging::judge_cases;
pub use kind::JobKind;
pub use pack::{Pack, read_packs};
pub use queue::{Job, claim, fail, finish, report_case};
pub use report::CaseReport;
pub use sandbox::{Outcome, run_in_sandbox};
pub use schedule::read_schedule;
pub use stdio::case_verdict;
pub use submission::Submission;
pub use verdict::Verdict;
