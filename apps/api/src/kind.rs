//! The job a student's run becomes (ADR-0040)

use contracts::RunKind;

#[derive(Debug, Clone, Copy, sqlx::Type)]
#[sqlx(type_name = "job_kind", rename_all = "lowercase")]
pub(super) enum JobKind {
    Precheck,
    Check,
}

impl From<RunKind> for JobKind {
    fn from(kind: RunKind) -> Self {
        match kind {
            RunKind::Precheck => Self::Precheck,
            RunKind::Check => Self::Check,
        }
    }
}
