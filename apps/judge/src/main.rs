//! Pulls jobs, runs the sandbox and writes the verdict.

use std::{env, error::Error, future, io, path::Path, str::FromStr, time::Duration};

use judge::{Case, Job, Verdict, claim, fail, finish, judge_cases, read_cases};
use sqlx::{Connection, PgConnection};
use tokio::time;

#[derive(Debug, Clone, Copy)]
enum Pack {
    Python,
}

impl FromStr for Pack {
    type Err = io::Error;

    fn from_str(name: &str) -> io::Result<Self> {
        match name {
            "python" => Ok(Self::Python),
            _ => Err(io::Error::new(
                io::ErrorKind::Unsupported,
                format!("language pack {name}"),
            )),
        }
    }
}

impl Pack {
    fn image(self) -> &'static str {
        match self {
            Self::Python => "python:3.14-alpine",
        }
    }
}

// Python pack until packs are data (ADR-0013)
const TIMEOUT: Duration = Duration::from_secs(10);
const POLL: Duration = Duration::from_secs(1);

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    let database = env::var("DATABASE_URL")?;
    let content = env::var_os("CONTENT").ok_or("CONTENT is not set")?;
    let mut stop = stop_signal()?;
    let mut connection = PgConnection::connect(&database).await?;

    loop {
        tokio::select! {
            biased;
            _ = stop.recv() => return Ok(()),
            () = future::ready(()) => {}
        }

        let Some(job) = claim(&mut connection).await? else {
            time::sleep(POLL).await;
            continue;
        };

        // Job cannot be judged: failed so it is never reclaimed
        let (pack, cases) = match prepare(content.as_ref(), &job) {
            Ok(prepared) => prepared,
            Err(error) => {
                eprintln!("judge: job {}: {error}", job.id);
                if !fail(&mut connection, job.id).await? {
                    eprintln!("judge: job {}: was no longer running", job.id);
                }
                continue;
            }
        };

        // A sandbox failure stops the judge rather than failing every job it claims
        let verdicts = judge(&job, pack, &cases).await?;
        if !finish(&mut connection, job.id, &verdicts).await? {
            eprintln!("judge: job {} was no longer running", job.id);
        }
    }
}

#[cfg(unix)]
fn stop_signal() -> io::Result<tokio::signal::unix::Signal> {
    use tokio::signal::unix::{SignalKind, signal};
    signal(SignalKind::terminate())
}

#[cfg(windows)]
fn stop_signal() -> io::Result<tokio::signal::windows::CtrlC> {
    tokio::signal::windows::ctrl_c()
}

fn prepare(content: &Path, job: &Job) -> io::Result<(Pack, Vec<Case>)> {
    let pack = job.language_pack.parse()?;
    let assessment = content
        .join("exercises")
        .join(&job.exercise)
        .join("assessment");
    Ok((pack, read_cases(&assessment)?))
}

async fn judge(job: &Job, pack: Pack, cases: &[Case]) -> io::Result<Vec<Verdict>> {
    let name = format!("judge-{}", job.id);
    let command = ["python3", "-c", &job.source];
    judge_cases(&name, pack.image(), &command, cases, TIMEOUT).await
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

    fn job(language_pack: &str, exercise: &str, source: &str) -> Job {
        Job {
            id: Uuid::default(),
            contract: 1,
            course: "log791".to_string(),
            exercise: exercise.into(),
            language_pack: language_pack.into(),
            source: source.into(),
        }
    }

    #[test]
    fn unknown_language_pack_is_unsupported() {
        let result = prepare(content(), &job("invalid", "sum", ""));
        assert_matches!(result, Err(error) if error.kind() == io::ErrorKind::Unsupported);
    }

    #[test]
    fn unknown_exercise_is_not_found() {
        let result = prepare(content(), &job("python", "missing", ""));
        assert_matches!(result, Err(error) if error.kind() == io::ErrorKind::NotFound);
    }

    #[tokio::test]
    #[ignore = "needs Docker with runsc"]
    async fn reference_solution_passes_the_sum_exercise() {
        let sum = "import sys; print(sum(map(int, sys.stdin.read().split())))";
        let job = job("python", "sum", sum);
        let (pack, cases) = prepare(content(), &job).unwrap();
        let verdicts = judge(&job, pack, &cases).await.unwrap();
        assert_eq!(verdicts, [Verdict::Passed; 5]);
    }
}
