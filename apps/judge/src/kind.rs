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
    fn check_and_final_run_every_case() {
        for kind in [JobKind::Check, JobKind::Final] {
            assert!(kind.runs(Visible));
            assert!(kind.runs(Hidden));
        }
    }
}
