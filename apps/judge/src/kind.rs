//! What a job runs (ADR-0040)

use crate::Visibility;

#[derive(sqlx::Type, Debug, Clone, Copy, PartialEq, Eq)]
#[sqlx(type_name = "job_kind", rename_all = "kebab-case")]
pub enum JobKind {
    Precheck,
    Check,
    Final,
}

impl JobKind {
    pub fn runs(self, visibility: Visibility) -> bool {
        match self {
            Self::Precheck => visibility == Visibility::Visible,
            Self::Check | Self::Final => true,
        }
    }

    /// A hidden case is never reported, and nobody waits for a final job (ADR-0030)
    pub fn reports(self, visibility: Visibility) -> bool {
        self != Self::Final && visibility == Visibility::Visible
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Visibility::{Hidden, Visible};

    #[test]
    fn precheck_runs_only_visible_cases() {
        assert!(JobKind::Precheck.runs(Visible));
        assert!(!JobKind::Precheck.runs(Hidden));
    }

    #[test]
    fn only_the_visible_cases_of_a_test_run_are_reported() {
        for kind in [JobKind::Precheck, JobKind::Check] {
            assert!(kind.reports(Visible));
            assert!(!kind.reports(Hidden));
        }
    }

    #[test]
    fn a_final_job_reports_nothing() {
        assert!(!JobKind::Final.reports(Visible));
        assert!(!JobKind::Final.reports(Hidden));
    }

    #[test]
    fn check_and_final_run_every_case() {
        for kind in [JobKind::Check, JobKind::Final] {
            assert!(kind.runs(Visible));
            assert!(kind.runs(Hidden));
        }
    }
}
