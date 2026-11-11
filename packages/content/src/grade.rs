//! A contest grade: set by how many of the three problems are solved (ADR-0043)

use contracts::CaseVerdict;

#[derive(Debug)]
pub enum Final {
    Missing,
    Pending,
    Failed,
    Judged(Vec<CaseVerdict>),
}

const SCALE: [u8; 4] = [0, 50, 90, 100];

pub fn contest_grade(finals: &[Final; 3]) -> Option<u8> {
    let mut solved = 0;
    for last in finals {
        match last {
            Final::Missing => {}
            Final::Pending | Final::Failed => return None,
            Final::Judged(verdicts) => {
                if solves(verdicts) {
                    solved += 1;
                }
            }
        }
    }
    Some(SCALE[solved])
}

fn solves(verdicts: &[CaseVerdict]) -> bool {
    !verdicts.is_empty()
        && verdicts
            .iter()
            .all(|verdict| *verdict == CaseVerdict::Passed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use CaseVerdict::{Passed, WrongAnswer};

    fn passed() -> Final {
        Final::Judged(vec![Passed, Passed])
    }

    #[test]
    fn grade_follows_the_number_solved() {
        assert_eq!(
            contest_grade(&[Final::Missing, Final::Missing, Final::Missing]),
            Some(0)
        );
        assert_eq!(
            contest_grade(&[Final::Missing, passed(), Final::Missing]),
            Some(50)
        );
        assert_eq!(
            contest_grade(&[passed(), Final::Missing, passed()]),
            Some(90)
        );
        assert_eq!(contest_grade(&[passed(), passed(), passed()]), Some(100));
    }

    #[test]
    fn one_failed_hidden_case_leaves_a_problem_unsolved() {
        let partial = Final::Judged(vec![Passed, WrongAnswer]);
        assert_eq!(contest_grade(&[passed(), partial, passed()]), Some(90));
    }

    #[test]
    fn no_verdict_is_not_solved() {
        assert_eq!(
            contest_grade(&[Final::Judged(vec![]), passed(), passed()]),
            Some(90)
        );
    }

    #[test]
    fn pending_or_failed_job_blocks_the_grade() {
        assert_eq!(contest_grade(&[passed(), Final::Pending, passed()]), None);
        assert_eq!(contest_grade(&[Final::Failed, passed(), passed()]), None);
    }
}
