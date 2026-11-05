//! Submissions run on every standard I/O case of an exercise (ADR-0013, ADR-0018)

use std::{io, time::Duration};

use crate::{Case, Pack, Submission, Verdict, case_verdict, run_in_sandbox};

// One case at a time; concurrent sandboxes once V-0013 measures how many a judge holds
pub async fn judge_cases(
    name: &str,
    pack: &Pack,
    source: &str,
    cases: &[Case],
    timeout: Duration,
) -> io::Result<Vec<Verdict>> {
    let submission = Submission::write(name, &pack.source, source)?;
    let mut verdicts = Vec::with_capacity(cases.len());
    for (index, case) in cases.iter().enumerate() {
        let container = format!("{name}-{index}");
        let outcome = run_in_sandbox(&container, pack, &submission, &case.input, timeout).await?;
        verdicts.push(case_verdict(&outcome, &case.expected));
    }
    Ok(verdicts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{read_cases, read_packs};
    use std::{env, path::Path};

    const TIMEOUT: Duration = Duration::from_mins(1);
    const SUM: &str = "import sys; print(sum(map(int, sys.stdin.read().split())))";

    async fn python_on_sum(name: &str, code: &str) -> Vec<Verdict> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let cases = read_cases(&root.join("examples/content/exercises/sum/assessment")).unwrap();
        let packs = read_packs(&root.join("packs/languages")).unwrap();
        let python = &packs[&"python".parse().unwrap()];
        let name = format!("judge-test-{name}");
        let verdicts = judge_cases(&name, python, code, &cases, TIMEOUT)
            .await
            .unwrap();
        assert!(!env::temp_dir().join(&name).exists(), "code left behind");
        verdicts
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

    #[tokio::test]
    #[ignore = "needs Docker with runsc"]
    async fn the_largest_source_accepted_is_judged() {
        let code = format!("{SUM}\n{}", "#".repeat(1024 * 1024 - SUM.len() - 1));
        assert_eq!(python_on_sum("large", &code).await, [Verdict::Passed; 5]);
    }
}
