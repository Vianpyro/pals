//! The visible cases of a student's runs, each reported as soon as it ends (ADR-0030)

use axum::http::StatusCode;
use contracts::{CaseResult, CaseVerdict};
use sqlx::PgPool;
use uuid::Uuid;

use super::runs::internal_error;

#[derive(Debug, Clone, Copy, sqlx::Type)]
#[sqlx(type_name = "case_verdict", rename_all = "kebab-case")]
enum Verdict {
    Passed,
    WrongAnswer,
    Crashed,
    TimedOut,
}

impl From<Verdict> for CaseVerdict {
    fn from(verdict: Verdict) -> Self {
        match verdict {
            Verdict::Passed => Self::Passed,
            Verdict::WrongAnswer => Self::WrongAnswer,
            Verdict::Crashed => Self::Crashed,
            Verdict::TimedOut => Self::TimedOut,
        }
    }
}

// The column's CHECK keeps the position non-negative: a broken database is a 500 not a guess
fn case(
    run: Uuid,
    position: i32,
    verdict: Verdict,
    actual: String,
    truncated: bool,
) -> Result<CaseResult, StatusCode> {
    Ok(CaseResult {
        run,
        position: u32::try_from(position).map_err(|error| internal_error("case", error))?,
        verdict: verdict.into(),
        actual,
        truncated,
    })
}

pub(super) async fn one(
    pool: &PgPool,
    run: Uuid,
    position: u32,
    author: &str,
) -> Result<Option<CaseResult>, StatusCode> {
    let position = i32::try_from(position).map_err(|error| internal_error("case", error))?;
    sqlx::query!(
        r#"
            SELECT job_cases.job AS run, position, verdict AS "verdict: Verdict", actual, truncated
            FROM job_cases JOIN runs ON runs.id = job_cases.job
            WHERE job_cases.job = $1 AND position = $2 AND runs.author = $3
        "#,
        run,
        position,
        author
    )
    .fetch_optional(pool)
    .await
    .map_err(|error| internal_error("case", error))?
    .map(|row| {
        case(
            row.run,
            row.position,
            row.verdict,
            row.actual,
            row.truncated,
        )
    })
    .transpose()
}

pub(super) async fn of(pool: &PgPool, runs: &[Uuid]) -> Result<Vec<CaseResult>, StatusCode> {
    sqlx::query!(
        r#"
            SELECT job AS run, position, verdict AS "verdict: Verdict", actual, truncated
            FROM job_cases WHERE job = ANY($1)
            ORDER BY job, position
        "#,
        runs
    )
    .fetch_all(pool)
    .await
    .map_err(|error| internal_error("cases", error))?
    .into_iter()
    .map(|row| {
        case(
            row.run,
            row.position,
            row.verdict,
            row.actual,
            row.truncated,
        )
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negative_position_is_a_500() {
        assert_eq!(
            case(Uuid::nil(), -1, Verdict::Passed, String::new(), false),
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        );
    }

    #[test]
    fn verdicts_keep_their_meaning() {
        let pairs = [
            (Verdict::Passed, CaseVerdict::Passed),
            (Verdict::WrongAnswer, CaseVerdict::WrongAnswer),
            (Verdict::Crashed, CaseVerdict::Crashed),
            (Verdict::TimedOut, CaseVerdict::TimedOut),
        ];
        for (verdict, expected) in pairs {
            assert_eq!(CaseVerdict::from(verdict), expected);
        }
    }
}
