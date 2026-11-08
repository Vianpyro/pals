//! Submissions run on every standard I/O case of an exercise (ADR-0013, ADR-0018)

use std::{io, time::Duration};

use crate::{Case, CaseReport, JobKind, Pack, Submission, Verdict, case_verdict, run_in_sandbox};

// One case at a time; concurrent sandboxes once V-0013 measures how many a judge holds
pub async fn judge_cases(
    name: &str,
    kind: JobKind,
    pack: &Pack,
    source: &str,
    cases: &[Case],
    timeout: Duration,
    mut report: impl AsyncFnMut(CaseReport) -> io::Result<()>,
) -> io::Result<Vec<Verdict>> {
    let submission = Submission::write(name, &pack.source, source)?;
    let mut verdicts = Vec::with_capacity(cases.len());
    for (position, case) in (0..).zip(cases) {
        let container = format!("{name}-{position}");
        let outcome = run_in_sandbox(&container, pack, &submission, &case.input, timeout).await?;
        let verdict = case_verdict(&outcome, &case.expected);
        if kind.reports(case.visibility) {
            report(CaseReport::new(position, verdict, &outcome)).await?;
        }
        verdicts.push(verdict)
    }
    Ok(verdicts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{read_cases, read_packs};
    use std::{env, path::Path, time::Instant};

    const TIMEOUT: Duration = Duration::from_mins(1);
    const SUM: &str = "import sys; print(sum(map(int, sys.stdin.read().split())))";

    async fn on_sum(
        name: &str,
        kind: JobKind,
        code: &str,
        report: impl AsyncFnMut(CaseReport) -> io::Result<()>,
    ) -> Vec<Verdict> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let cases = read_cases(&root.join("examples/content/exercises/sum/assessment")).unwrap();
        let packs = read_packs(&root.join("packs/languages")).unwrap();
        let python = &packs[&"python".parse().unwrap()];
        let name = format!("judge-test-{name}");
        let verdicts = judge_cases(&name, kind, python, code, &cases, TIMEOUT, report)
            .await
            .unwrap();
        assert!(!env::temp_dir().join(&name).exists(), "code left behind");
        verdicts
    }

    async fn python_on_sum(name: &str, code: &str) -> Vec<Verdict> {
        on_sum(name, JobKind::Check, code, async |_| Ok(())).await
    }

    #[tokio::test]
    #[ignore = "needs Docker with runsc"]
    async fn check_reports_only_visible() {
        let mut reported = Vec::new();
        let verdicts = on_sum("reports-visible", JobKind::Check, SUM, async |report| {
            reported.push(report);
            Ok(())
        })
        .await;
        assert_eq!(verdicts, [Verdict::Passed; 5]);
        let positions: Vec<i32> = reported.iter().map(|report| report.position).collect();
        assert_eq!(positions, [0, 1]);
        assert!(reported.iter().all(|report| !report.actual.is_empty()));
    }

    #[tokio::test]
    #[ignore = "needs Docker with runsc"]
    async fn a_final_job_reports_no_case() {
        let mut reported = 0;
        let verdicts = on_sum("reports-nothing", JobKind::Final, SUM, async |_| {
            reported += 1;
            Ok(())
        })
        .await;
        assert_eq!(verdicts, [Verdict::Passed; 5]);
        assert_eq!(reported, 0);
    }

    #[tokio::test]
    #[ignore = "needs Docker with runsc"]
    async fn case_reported_before_next_runs() {
        let slow = "import time; time.sleep(1); print(0)";
        let mut ended = Vec::new();
        on_sum("reports-early", JobKind::Check, slow, async |_| {
            ended.push(Instant::now());
            Ok(())
        })
        .await;
        assert!(ended[1] - ended[0] >= Duration::from_secs(1));
    }

    #[tokio::test]
    #[ignore = "needs Docker with runsc"]
    async fn reference_solution_passes_every_case() {
        assert_eq!(python_on_sum("reference", SUM).await, [Verdict::Passed; 5]);
    }

    #[tokio::test]
    #[ignore = "needs Docker with runsc"]
    async fn wrong_solution_fails_every_case() {
        assert_eq!(
            python_on_sum("wrong", "print(0)").await,
            [Verdict::WrongAnswer; 5]
        );
    }

    // 125 is also Docker's own failure code: the program's own is still a crash
    #[tokio::test]
    #[ignore = "needs Docker with runsc"]
    async fn exit_125_crashes_every_case() {
        assert_eq!(
            python_on_sum("crash-125", "import sys; sys.exit(125)").await,
            [Verdict::Crashed; 5]
        );
    }

    #[tokio::test]
    #[ignore = "needs Docker with runsc"]
    async fn largest_source_is_judged() {
        let code = format!("{SUM}\n{}", "#".repeat(1024 * 1024 - SUM.len() - 1));
        assert_eq!(python_on_sum("large", &code).await, [Verdict::Passed; 5]);
    }
}
