//! Pulls jobs, runs the sandbox and writes the verdict.

use std::{env, error::Error, future, io, path::Path, str::FromStr, time::Duration};

use content::{Schedule, accessible_exercise};
use contracts::Id;
use judge::{Case, Job, Verdict, claim, fail, finish, judge_cases, read_cases, read_schedule};
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
    let courses = env::var_os("COURSES").ok_or("COURSES is not set")?;
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

        // A database error stops the judge: the job is reclaimed once its lease expires
        let schedule = read_schedule(&mut connection, &job.course, &job.exercise).await?;

        // Job cannot be judged: failed so it is never reclaimed
        let (pack, cases) = match prepare(content.as_ref(), courses.as_ref(), &job, &schedule) {
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

fn prepare(
    content: &Path,
    courses: &Path,
    job: &Job,
    schedule: &Schedule,
) -> io::Result<(Pack, Vec<Case>)> {
    let pack = job.language_pack.parse()?;
    let course = id(&job.course)?;
    let exercise = id(&job.exercise)?;

    accessible_exercise(
        &courses.join(course.as_str()),
        &exercise,
        schedule,
        job.enqueued_at.into(),
    )?;

    let assessment = content
        .join(course.as_str())
        .join("exercises")
        .join(exercise.as_str())
        .join("assessment");
    Ok((pack, read_cases(&assessment)?))
}

fn id(value: &str) -> io::Result<Id> {
    value
        .parse()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

async fn judge(job: &Job, pack: Pack, cases: &[Case]) -> io::Result<Vec<Verdict>> {
    let name = format!("judge-{}", job.id);
    let command = ["python3", "-c", &job.source];
    judge_cases(&name, pack.image(), &command, cases, TIMEOUT).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use content::Window;
    use sqlx::types::{Uuid, time::OffsetDateTime};
    use std::{assert_matches, fs, path::PathBuf, time::UNIX_EPOCH};

    fn examples() -> &'static Path {
        Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/content"
        ))
    }

    // log200 with sum published in its active release and one case in its content repository
    fn log200(name: &str) -> (PathBuf, PathBuf) {
        let root = env::temp_dir().join("judge-tests").join(name);
        let content = root.join("content");
        let assessment = content.join("log200/exercises/sum/assessment");
        fs::create_dir_all(&assessment).unwrap();
        fs::write(assessment.join("1.in"), "1 2").unwrap();
        fs::write(assessment.join("1.out"), "3").unwrap();

        let courses = root.join("courses");
        fs::create_dir_all(courses.join("log200/releases/abc123/exercises/sum")).unwrap();
        fs::write(courses.join("log200/current"), "abc123").unwrap();
        (content, courses)
    }

    // Submitted 30 s after the epoch
    fn job(language_pack: &str, exercise: &str, source: &str) -> Job {
        Job {
            id: Uuid::default(),
            contract: 1,
            course: "log200".to_string(),
            exercise: exercise.into(),
            language_pack: language_pack.into(),
            source: source.into(),
            enqueued_at: OffsetDateTime::UNIX_EPOCH + Duration::from_secs(30),
        }
    }

    // Opens at the epoch
    fn closing_after(seconds: u64) -> Schedule {
        Schedule {
            offering: Some(Window {
                opens_at: UNIX_EPOCH,
                closes_at: Some(UNIX_EPOCH + Duration::from_secs(seconds)),
            }),
            ..Default::default()
        }
    }

    #[test]
    fn job_submitted_before_closing_is_judged_after_it() {
        let (content, courses) = log200("submitted-before-closing");
        let (_, cases) = prepare(
            &content,
            &courses,
            &job("python", "sum", ""),
            &closing_after(60),
        )
        .unwrap();
        assert_eq!(cases.len(), 1);
    }

    #[test]
    fn job_submitted_after_closing_is_denied() {
        let (content, courses) = log200("submitted-after-closing");
        let result = prepare(
            &content,
            &courses,
            &job("python", "sum", ""),
            &closing_after(10),
        );
        assert_matches!(result, Err(error) if error.kind() == io::ErrorKind::PermissionDenied);
    }

    #[test]
    fn unknown_language_pack_is_unsupported() {
        let (content, courses) = log200("unknown-pack");
        let result = prepare(
            &content,
            &courses,
            &job("invalid", "sum", ""),
            &closing_after(60),
        );
        assert_matches!(result, Err(error) if error.kind() == io::ErrorKind::Unsupported);
    }

    #[test]
    fn unpublished_exercise_is_not_found() {
        let (content, courses) = log200("unpublished");
        let result = prepare(
            &content,
            &courses,
            &job("python", "missing", ""),
            &closing_after(60),
        );
        assert_matches!(result, Err(error) if error.kind() == io::ErrorKind::NotFound);
    }

    #[test]
    fn another_courses_cases_are_never_read() {
        let (content, courses) = log200("another-course");
        let mut job = job("python", "sum", "");
        job.course = "log201".to_string();
        let result = prepare(&content, &courses, &job, &closing_after(60));
        assert_matches!(result, Err(error) if error.kind() == io::ErrorKind::NotFound);
    }

    #[tokio::test]
    #[ignore = "needs Docker with runsc"]
    async fn reference_solution_passes_the_sum_exercise() {
        let sum = "import sys; print(sum(map(int, sys.stdin.read().split())))";
        let job = job("python", "sum", sum);
        let cases = read_cases(&examples().join("exercises/sum/assessment")).unwrap();
        let verdicts = judge(&job, Pack::Python, &cases).await.unwrap();
        assert_eq!(verdicts, [Verdict::Passed; 5]);
    }
}
