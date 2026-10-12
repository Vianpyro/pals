//! Pulls jobs, runs the sandbox and writes the verdict.

use std::{env, error::Error, io, path::Path, time::Duration};

use judge::{Case, Job, Verdict, claim, finish, judge_cases, read_cases};
use sqlx::{Connection, PgConnection};
use tokio::time;

// Python pack until packs are data (ADR-0013)
const IMAGE: &str = "python:3.14-alpine";
const TIMEOUT: Duration = Duration::from_secs(10);
const POLL: Duration = Duration::from_secs(1);

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    let database = env::var("DATABASE_URL")?;
    let content = env::var_os("CONTENT").ok_or("CONTENT is not set")?;
    let mut connection = PgConnection::connect(&database).await?;

    loop {
        let Some(job) = claim(&mut connection).await? else {
            time::sleep(POLL).await;
            continue;
        };

        match judge(content.as_ref(), &job).await {
            Ok(verdicts) => {
                if !finish(&mut connection, job.id, &verdicts).await? {
                    eprintln!("judge: job {} was no longer running", job.id);
                }
            }
            Err(error) => eprintln!("judge: job {}: {error}", job.id),
        }
    }
}

fn prepare(content: &Path, job: &Job) -> io::Result<(&'static str, Vec<Case>)> {
    let image = match job.language_pack.as_str() {
        "python" => IMAGE,
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                format!("language pack {}", job.language_pack),
            ));
        }
    };
    let assessment = content
        .join("exercises")
        .join(&job.exercise)
        .join("assessment");
    Ok((image, read_cases(&assessment)?))
}

async fn judge(content: &Path, job: &Job) -> io::Result<Vec<Verdict>> {
    let (image, cases) = prepare(content, job)?;
    let name = format!("judge-{}", job.id);
    let command = ["python3", "-c", &job.source];
    judge_cases(&name, image, &command, &cases, TIMEOUT).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::types::Uuid;
    use std::assert_matches;

    fn content() -> &'static Path {
        Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/content"
        ))
    }

    fn job(language_pack: &str, source: &str) -> Job {
        Job {
            id: Uuid::default(),
            contract: 1,
            course: "log791".to_string(),
            exercise: "sum".into(),
            language_pack: language_pack.into(),
            source: source.into(),
        }
    }

    #[tokio::test]
    async fn unknown_language_pack_is_unsupported() {
        let result = judge(content(), &job("invalid", "")).await;
        assert_matches!(result, Err(error) if error.kind() == io::ErrorKind::Unsupported);
    }

    #[tokio::test]
    #[ignore = "needs Docker with runsc"]
    async fn reference_solution_passes_the_sum_exercise() {
        let sum = "import sys; print(sum(map(int, sys.stdin.read().split())))";
        let verdicts = judge(content(), &job("python", sum)).await.unwrap();
        assert_eq!(verdicts, [Verdict::Passed; 5]);
    }
}
