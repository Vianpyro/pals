//! Pulls jobs, runs the sandbox and writes the verdict.

use std::{
    collections::HashMap,
    env,
    error::Error,
    future, io,
    path::{Path, PathBuf},
    time::Duration,
};

use content::{Schedule, accessible_exercise};
use contracts::Id;
use judge::{
    AnswerError, Case, CaseReport, ChoiceKey, Job, JobKind, Pack, Verdict, claim, enqueue_finals,
    fail, finish, finish_choice, grade, judge_cases, read_answer, read_cases, read_choice_key,
    read_packs, read_schedule, report_case,
};
use sqlx::{Connection, PgConnection, types::Uuid};
use tokio::time;

const TIMEOUT: Duration = Duration::from_secs(10);
const POLL: Duration = Duration::from_secs(1);

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    let database = env::var("DATABASE_URL")?;
    let content = env::var_os("CONTENT").ok_or("CONTENT is not set")?;
    let courses = env::var_os("COURSES").ok_or("COURSES is not set")?;
    let packs = env::var_os("PACKS").ok_or("PACKS is not set")?;
    let packs = read_packs(packs.as_ref())?;
    let mut stop = stop_signal()?;
    let mut connection = PgConnection::connect(&database).await?;

    loop {
        tokio::select! {
            biased;
            _ = stop.recv() => return Ok(()),
            () = future::ready(()) => {}
        }

        let Some(job) = claim(&mut connection).await? else {
            if enqueue_finals(&mut connection).await? == 0 {
                time::sleep(POLL).await;
            }
            continue;
        };

        // A database error stops the judge: the job is reclaimed once its lease expires
        let schedule = read_schedule(&mut connection, &job.offering, &job.exercise).await?;

        // Job cannot be judged: failed so it is never reclaimed
        let assessment = match assessment(content.as_ref(), courses.as_ref(), &job, &schedule) {
            Ok(path) => path,
            Err(error) => {
                reject(&mut connection, job.id, error).await?;
                continue;
            }
        };

        match read_choice_key(&assessment) {
            Ok(key) => {
                match choice_score(&job, &key) {
                    Ok(score) => {
                        if !finish_choice(&mut connection, job.id, score).await? {
                            eprintln!("judge: job {} was no longer running", job.id);
                        }
                    }
                    Err(error) => reject(&mut connection, job.id, error).await?,
                }
                continue;
            }
            // No choice key: the exercise is judged code
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                reject(&mut connection, job.id, error).await?;
                continue;
            }
        }

        let (pack, cases) = match prepare(&packs, &assessment, &job) {
            Ok(prepared) => prepared,
            Err(error) => {
                reject(&mut connection, job.id, error).await?;
                continue;
            }
        };

        // A sandbox failure stops the judge rather than failing every job it claims
        let verdicts = judge(&job, pack, &cases, async |report| {
            report_case(&mut connection, job.id, &report)
                .await
                .map_err(io::Error::other)?;
            Ok(())
        })
        .await?;
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

fn prepare<'p>(
    packs: &'p HashMap<Id, Pack>,
    assessment: &Path,
    job: &Job,
) -> io::Result<(&'p Pack, Vec<Case>)> {
    let pack = packs.get(&id(&job.language_pack)?).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::Unsupported,
            format!("language pack {}", job.language_pack),
        )
    })?;
    let mut cases = read_cases(assessment)?;
    cases.retain(|case| job.kind.runs(case.visibility));
    (!cases.is_empty()).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("no case for a {:?} job", job.kind),
        )
    })?;

    Ok((pack, cases))
}

fn assessment(
    content: &Path,
    courses: &Path,
    job: &Job,
    schedule: &Schedule,
) -> io::Result<PathBuf> {
    let course = id(&job.course)?;
    let exercise = id(&job.exercise)?;

    accessible_exercise(
        &courses.join(course.as_str()),
        &exercise,
        schedule,
        job.enqueued_at.into(),
    )?;

    Ok(content
        .join(course.as_str())
        .join("exercises")
        .join(exercise.as_str())
        .join("assessment"))
}

fn choice_score(job: &Job, key: &ChoiceKey) -> io::Result<u8> {
    if job.kind != JobKind::Final {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "a choice item is graded only by a final job",
        ));
    }
    let malformed =
        |error: AnswerError| io::Error::new(io::ErrorKind::InvalidData, format!("{error:?}"));
    let selected = read_answer(&job.source).map_err(malformed)?;
    grade(key, &selected).map_err(malformed)
}

fn id(value: &str) -> io::Result<Id> {
    value
        .parse()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

async fn judge(
    job: &Job,
    pack: &Pack,
    cases: &[Case],
    report: impl AsyncFnMut(CaseReport) -> io::Result<()>,
) -> io::Result<Vec<Verdict>> {
    let name = format!("judge-{}", job.id);
    judge_cases(&name, job.kind, pack, &job.source, cases, TIMEOUT, report).await
}

async fn reject(connection: &mut PgConnection, job: Uuid, error: io::Error) -> sqlx::Result<()> {
    eprintln!("judge: job {job}: {error}");
    if !fail(connection, job).await? {
        eprintln!("judge: job {job}: was no longer running");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use content::Window;
    use judge::{Selection, Visibility};
    use sqlx::types::time::OffsetDateTime;
    use std::{assert_matches, fs, time::UNIX_EPOCH};

    fn examples() -> &'static Path {
        Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/content"
        ))
    }

    // log200 with sum published in its active release and one hidden case in its content repository
    fn log200(name: &str) -> (PathBuf, PathBuf) {
        let root = env::temp_dir().join("judge-tests").join(name);
        let _ = fs::remove_dir_all(&root); // left by a previous run
        let content = root.join("content");
        let assessment = content.join("log200/exercises/sum/assessment/hidden");
        fs::create_dir_all(&assessment).unwrap();
        fs::write(assessment.join("1.in"), "1 2").unwrap();
        fs::write(assessment.join("1.out"), "3").unwrap();

        let courses = root.join("courses");
        fs::create_dir_all(courses.join("log200/releases/abc123/exercises/sum")).unwrap();
        fs::write(courses.join("log200/current"), "abc123").unwrap();
        (content, courses)
    }

    fn packs() -> HashMap<Id, Pack> {
        read_packs(Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packs/languages"
        )))
        .unwrap()
    }

    // Submitted 30 s after the epoch
    fn job(language_pack: &str, exercise: &str, source: &str) -> Job {
        Job {
            id: Uuid::default(),
            contract: 1,
            kind: JobKind::Check,
            offering: "log200".to_string(),
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

    fn prepare_job<'p>(
        packs: &'p HashMap<Id, Pack>,
        content: &Path,
        courses: &Path,
        job: &Job,
        schedule: &Schedule,
    ) -> io::Result<(&'p Pack, Vec<Case>)> {
        prepare(packs, &assessment(content, courses, job, schedule)?, job)
    }

    fn choice(selection: Selection, weights: &[i32]) -> ChoiceKey {
        ChoiceKey {
            selection,
            weights: weights.to_vec(),
        }
    }

    fn final_answer(source: &str) -> Job {
        let mut job = job("python", "sum", source);
        job.kind = JobKind::Final;
        job
    }

    #[test]
    fn final_job_grades_a_choice_item() {
        let key = choice(Selection::Multiple, &[50, 50, -100]);
        assert_eq!(choice_score(&final_answer("0"), &key).unwrap(), 50);
    }

    #[test]
    fn test_runs_never_grade_a_choice_item() {
        let key = choice(Selection::Single, &[0, 100]);
        for kind in [JobKind::Precheck, JobKind::Check] {
            let mut job = final_answer("1");
            job.kind = kind;
            assert_matches!(choice_score(&job, &key), Err(error) if error.kind() == io::ErrorKind::Unsupported);
        }
    }

    #[test]
    fn invalid_choice_answer_is_invalid_data() {
        let key = choice(Selection::Single, &[0, 100]);
        for source in ["x", "5", "0,1"] {
            assert_matches!(
                choice_score(&final_answer(source), &key),
                Err(error) if error.kind() == io::ErrorKind::InvalidData,
                "{source:?}"
            );
        }
    }

    #[test]
    fn closed_choice_item_is_denied() {
        let (content, courses) = log200("choice-after-closing");
        let result = assessment(&content, &courses, &final_answer("1"), &closing_after(10));
        assert_matches!(result, Err(error) if error.kind() == io::ErrorKind::PermissionDenied);
    }

    #[test]
    fn submitted_before_close_is_judged() {
        let (content, courses) = log200("submitted-before-closing");
        let packs = packs();
        let (_, cases) = prepare_job(
            &packs,
            &content,
            &courses,
            &job("python", "sum", ""),
            &closing_after(60),
        )
        .unwrap();
        assert_eq!(cases.len(), 1);
    }

    #[test]
    fn precheck_keeps_only_visible() {
        let (content, courses) = log200("precheck-visible");
        let visible = content.join("log200/exercises/sum/assessment/visible");
        fs::create_dir_all(&visible).unwrap();
        fs::write(visible.join("1.in"), "2 2").unwrap();
        fs::write(visible.join("1.out"), "4").unwrap();
        let mut job = job("python", "sum", "");
        job.kind = JobKind::Precheck;
        let packs = packs();
        let (_, cases) = prepare_job(&packs, &content, &courses, &job, &closing_after(60)).unwrap();
        let kinds: Vec<Visibility> = cases.iter().map(|case| case.visibility).collect();
        assert_eq!(kinds, [Visibility::Visible]);
    }

    #[test]
    fn precheck_needs_a_visible_case() {
        let (content, courses) = log200("precheck-hidden-only");
        let mut job = job("python", "sum", "");
        job.kind = JobKind::Precheck;
        let packs = packs();
        let result = prepare_job(&packs, &content, &courses, &job, &closing_after(60));
        assert_matches!(result, Err(error) if error.kind() == io::ErrorKind::InvalidData);
    }

    #[test]
    fn submitted_after_close_is_denied() {
        let (content, courses) = log200("submitted-after-closing");
        let packs = packs();
        let result = prepare_job(
            &packs,
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
        let packs = packs();
        let result = prepare_job(
            &packs,
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
        let packs = packs();
        let result = prepare_job(
            &packs,
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
        let packs = packs();
        let result = prepare_job(&packs, &content, &courses, &job, &closing_after(60));
        assert_matches!(result, Err(error) if error.kind() == io::ErrorKind::NotFound);
    }

    #[tokio::test]
    #[ignore = "needs Docker with runsc"]
    async fn reference_passes_sum() {
        let sum = "import sys; print(sum(map(int, sys.stdin.read().split())))";
        let job = job("python", "sum", sum);
        let cases = read_cases(&examples().join("exercises/sum/assessment")).unwrap();
        let verdicts = judge(&job, &packs()[&id("python").unwrap()], &cases, async |_| {
            Ok(())
        })
        .await
        .unwrap();
        assert_eq!(verdicts, [Verdict::Passed; 5]);
    }
}
