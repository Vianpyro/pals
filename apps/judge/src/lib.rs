//! Judging of a submission in a gVisor sandbox (ADR-0002, ADR-0018)

mod cases;
mod judging;
mod queue;
mod sandbox;
mod stdio;
mod verdict;

pub use cases::{Case, read_cases};
pub use judging::judge_cases;
pub use queue::{Job, claim, fail, finish};
pub use sandbox::{Outcome, run_in_sandbox};
pub use stdio::case_verdict;
pub use verdict::Verdict;
