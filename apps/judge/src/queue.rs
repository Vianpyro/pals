//! Jobs claimed from the PostgreSQL queue (ADR-0001)

use sqlx::{
    PgConnection,
    types::{Uuid, time::OffsetDateTime},
};

use crate::{CaseReport, JobKind, Verdict};

#[derive(Debug)]
pub struct Job {
    pub id: Uuid,
    pub contract: i32,
    pub kind: JobKind,
    pub course: String,
    pub exercise: String,
    pub language_pack: String,
    pub source: String,
    pub enqueued_at: OffsetDateTime,
}

pub async fn claim(connection: &mut PgConnection) -> sqlx::Result<Option<Job>> {
    // RETURNS TABLE hides nullability from the macros: every column comes from a NOT NULL column of jobs
    sqlx::query_as!(
        Job,
        r#"
            SELECT
                id AS "id!",
                contract AS "contract!",
                kind AS "kind!: JobKind",
                course AS "course!: String",
                exercise AS "exercise!: String",
                language_pack AS "language_pack!: String",
                source AS "source!",
                enqueued_at AS "enqueued_at!"
            FROM claim_job()
        "#
    )
    .fetch_optional(connection)
    .await
}

pub async fn finish(
    connection: &mut PgConnection,
    job: Uuid,
    verdicts: &[Verdict],
) -> sqlx::Result<bool> {
    sqlx::query_scalar!(
        r#"SELECT finish_job($1, $2) AS "finished!""#,
        job,
        verdicts as &[Verdict]
    )
    .fetch_one(connection)
    .await
}

pub async fn finish_choice(
    connection: &mut PgConnection,
    job: Uuid,
    score: u8,
) -> sqlx::Result<bool> {
    sqlx::query_scalar!(
        r#"SELECT finish_choice($1, $2) AS "finished!""#,
        job,
        i16::from(score),
    )
    .fetch_one(connection)
    .await
}

pub async fn report_case(
    connection: &mut PgConnection,
    job: Uuid,
    report: &CaseReport,
) -> sqlx::Result<bool> {
    sqlx::query_scalar!(
        r#"SELECT report_case($1, $2, $3, $4, $5) AS "reported!""#,
        job,
        report.position,
        report.verdict as Verdict,
        report.actual,
        report.truncated
    )
    .fetch_one(connection)
    .await
}

pub async fn fail(connection: &mut PgConnection, job: Uuid) -> sqlx::Result<bool> {
    sqlx::query_scalar!(r#"SELECT fail_job($1) AS "failed!""#, job)
        .fetch_one(connection)
        .await
}

pub async fn enqueue_finals(connection: &mut PgConnection) -> sqlx::Result<i32> {
    sqlx::query_scalar!(r#"SELECT enqueue_finals() AS "queued!""#)
        .fetch_one(connection)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::{PgPool, Postgres, Transaction};
    use std::time::Duration;
    use tokio::time;

    const AS_JUDGE: &str = "SET LOCAL ROLE judge";
    const AS_API: &str = "SET LOCAL ROLE api";

    async fn enqueue(pool: &PgPool) -> Uuid {
        sqlx::query_scalar(
            r#"
                INSERT INTO jobs (contract, course, exercise, language_pack, source, author)
                VALUES (1, 'log200', 'sum', 'python', 'print(0)', 'etudiant1')
                RETURNING id
            "#,
        )
        .fetch_one(pool)
        .await
        .unwrap()
    }

    async fn enqueue_kind(pool: &PgPool, kind: JobKind) {
        sqlx::query(
            r#"
                INSERT INTO jobs (contract, kind, course, exercise, language_pack, source, author)
                VALUES (1, $1, 'log200', 'sum', 'python', 'print(0)', 'etudiant1')
            "#,
        )
        .bind(kind)
        .execute(pool)
        .await
        .unwrap();
    }

    // Tests connect as a superuser: each check runs with the role's privileges instead
    async fn begin_as(pool: &PgPool, role: &'static str) -> Transaction<'static, Postgres> {
        let mut transaction = pool.begin().await.unwrap();
        sqlx::query(role).execute(&mut *transaction).await.unwrap();
        transaction
    }

    async fn claim_as_judge(pool: &PgPool) -> Option<Uuid> {
        let mut transaction = begin_as(pool, AS_JUDGE).await;
        let job = claim(&mut transaction).await.unwrap();
        transaction.commit().await.unwrap();
        job.map(|job| job.id)
    }

    async fn claimed_kind(pool: &PgPool) -> JobKind {
        let mut transaction = begin_as(pool, AS_JUDGE).await;
        let job = claim(&mut transaction).await.unwrap().unwrap();
        transaction.commit().await.unwrap();
        job.kind
    }

    async fn finish_as_judge(pool: &PgPool, job: Uuid, verdicts: &[Verdict]) -> bool {
        let mut transaction = begin_as(pool, AS_JUDGE).await;
        let finished = finish(&mut transaction, job, verdicts).await.unwrap();
        transaction.commit().await.unwrap();
        finished
    }

    async fn state_and_result(pool: &PgPool, job: Uuid) -> (String, Option<String>) {
        sqlx::query_as("SELECT state::text, result::text FROM jobs WHERE id = $1")
            .bind(job)
            .fetch_one(pool)
            .await
            .unwrap()
    }

    fn passed_at(position: i32) -> CaseReport {
        CaseReport {
            position,
            verdict: Verdict::Passed,
            actual: "3\n".to_string(),
            truncated: false,
        }
    }

    async fn report_as_judge(pool: &PgPool, job: Uuid, report: &CaseReport) -> bool {
        let mut transaction = begin_as(pool, AS_JUDGE).await;
        let reported = report_case(&mut transaction, job, report).await.unwrap();
        transaction.commit().await.unwrap();
        reported
    }

    async fn reported_cases(pool: &PgPool, job: Uuid) -> Vec<(i32, String)> {
        sqlx::query_as(
            "SELECT position, verdict::text FROM job_cases WHERE job = $1 ORDER BY position",
        )
        .bind(job)
        .fetch_all(pool)
        .await
        .unwrap()
    }

    async fn schedule_sum(pool: &PgPool, closes_in: &str) {
        sqlx::query(
            "INSERT INTO schedules VALUES ('log200', 'sum', now() - interval '1 day', now() + $1::interval)",
        )
        .bind(closes_in)
        .execute(pool)
        .await
        .unwrap();
    }

    async fn save_draft(pool: &PgPool, author: &str, source: &str) {
        sqlx::query(
            r#"
                INSERT INTO drafts (author, course, exercise, language_pack, revision, source)
                VALUES ($1, 'log200', 'sum', 'python', 1, $2)
            "#,
        )
        .bind(author)
        .bind(source)
        .execute(pool)
        .await
        .unwrap();
    }

    async fn enqueue_finals_as_judge(pool: &PgPool) -> i32 {
        let mut transaction = begin_as(pool, AS_JUDGE).await;
        let queued = enqueue_finals(&mut transaction).await.unwrap();
        transaction.commit().await.unwrap();
        queued
    }

    #[track_caller]
    fn assert_denied(error: &sqlx::Error) {
        let code = error.as_database_error().and_then(|error| error.code());
        assert_eq!(code.as_deref(), Some("42501"), "{error}"); // SQLSTATE insufficient_privilege
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn empty_queue_has_nothing_to_claim(pool: PgPool) {
        assert_eq!(claim_as_judge(&pool).await, None);
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn claimed_oldest_first_once(pool: PgPool) {
        let first = enqueue(&pool).await;
        let second = enqueue(&pool).await;
        assert_eq!(claim_as_judge(&pool).await, Some(first));
        assert_eq!(claim_as_judge(&pool).await, Some(second));
        assert_eq!(claim_as_judge(&pool).await, None);
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn claimed_job_holds_its_kind(pool: PgPool) {
        for kind in [JobKind::Precheck, JobKind::Check, JobKind::Final] {
            enqueue_kind(&pool, kind).await;
            assert_eq!(claimed_kind(&pool).await, kind);
        }
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn missing_kind_is_a_check(pool: PgPool) {
        enqueue(&pool).await;
        assert_eq!(claimed_kind(&pool).await, JobKind::Check);
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn concurrent_judges_skip_a_locked_job(pool: PgPool) {
        let first = enqueue(&pool).await;
        let second = enqueue(&pool).await;
        let mut holding = begin_as(&pool, AS_JUDGE).await;
        let held = claim(&mut holding).await.unwrap().map(|job| job.id);
        // Without SKIP LOCKED, this waits for `holding` to end
        let other = time::timeout(Duration::from_secs(5), claim_as_judge(&pool))
            .await
            .expect("blocked behind a locked job");
        assert_eq!(held, Some(first));
        assert_eq!(other, Some(second));
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn judge_cannot_touch_jobs(pool: PgPool) {
        enqueue(&pool).await;
        for statement in [
            "SELECT source FROM jobs",
            "UPDATE jobs SET state = 'failed', finished_at = now()",
        ] {
            let mut transaction = begin_as(&pool, AS_JUDGE).await;
            let error = sqlx::query(statement)
                .execute(&mut *transaction)
                .await
                .unwrap_err();
            assert_denied(&error);
        }
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn only_the_judge_can_claim(pool: PgPool) {
        enqueue(&pool).await;
        let mut transaction = begin_as(&pool, AS_API).await;
        assert_denied(&claim(&mut transaction).await.unwrap_err());
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn claimed_job_is_finished_once(pool: PgPool) {
        let job = enqueue(&pool).await;
        claim_as_judge(&pool).await;
        let verdicts = [Verdict::Passed, Verdict::WrongAnswer];
        assert!(finish_as_judge(&pool, job, &verdicts).await);
        assert!(!finish_as_judge(&pool, job, &verdicts).await);
        let result = r#"{"cases": ["passed", "wrong-answer"]}"#;
        assert_eq!(
            state_and_result(&pool, job).await,
            ("done".into(), Some(result.into()))
        );
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn waiting_job_cannot_be_finished(pool: PgPool) {
        let job = enqueue(&pool).await;
        assert!(!finish_as_judge(&pool, job, &[Verdict::Passed]).await);
        assert_eq!(state_and_result(&pool, job).await, ("waiting".into(), None));
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn job_without_verdict_is_not_finished(pool: PgPool) {
        let job = enqueue(&pool).await;
        claim_as_judge(&pool).await;
        assert!(!finish_as_judge(&pool, job, &[]).await);
        assert_eq!(state_and_result(&pool, job).await, ("running".into(), None));
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn only_the_judge_can_finish(pool: PgPool) {
        let job = enqueue(&pool).await;
        claim_as_judge(&pool).await;
        let mut transaction = begin_as(&pool, AS_API).await;
        let error = finish(&mut transaction, job, &[Verdict::Passed])
            .await
            .unwrap_err();
        assert_denied(&error);
    }

    async fn enqueue_final(pool: &PgPool) -> Uuid {
        sqlx::query_scalar(
            r#"
                INSERT INTO jobs (contract, kind, course, exercise, language_pack, source, author)
                VALUES (1, 'final', 'log200', 'quiz', 'python', '1', 'etudiant1')
                RETURNING id
            "#,
        )
        .fetch_one(pool)
        .await
        .unwrap()
    }

    async fn finish_choice_as_judge(pool: &PgPool, job: Uuid, score: u8) -> bool {
        let mut transaction = begin_as(pool, AS_JUDGE).await;
        let finished = finish_choice(&mut transaction, job, score).await.unwrap();
        transaction.commit().await.unwrap();
        finished
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn final_job_keeps_its_choice_score(pool: PgPool) {
        let job = enqueue_final(&pool).await;
        claim_as_judge(&pool).await;
        assert!(finish_choice_as_judge(&pool, job, 75).await);
        assert!(!finish_choice_as_judge(&pool, job, 75).await);
        assert_eq!(
            state_and_result(&pool, job).await,
            ("done".into(), Some(r#"{"score": 75}"#.into()))
        );
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn check_job_cannot_keep_a_choice_score(pool: PgPool) {
        let job = enqueue(&pool).await;
        claim_as_judge(&pool).await;
        assert!(!finish_choice_as_judge(&pool, job, 100).await);
        assert_eq!(state_and_result(&pool, job).await, ("running".into(), None));
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn choice_score_above_100_is_refused(pool: PgPool) {
        let job = enqueue_final(&pool).await;
        claim_as_judge(&pool).await;
        assert!(!finish_choice_as_judge(&pool, job, 101).await);
        assert_eq!(state_and_result(&pool, job).await, ("running".into(), None));
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn only_the_judge_can_keep_a_choice_score(pool: PgPool) {
        let job = enqueue_final(&pool).await;
        claim_as_judge(&pool).await;
        let mut transaction = begin_as(&pool, AS_API).await;
        let error = finish_choice(&mut transaction, job, 100).await.unwrap_err();
        assert_denied(&error);
    }

    async fn fail_as_judge(pool: &PgPool, job: Uuid) -> bool {
        let mut transaction = begin_as(pool, AS_JUDGE).await;
        let failed = fail(&mut transaction, job).await.unwrap();
        transaction.commit().await.unwrap();
        failed
    }

    async fn expire(pool: &PgPool, job: Uuid) {
        sqlx::query("UPDATE jobs SET enqueued_at = enqueued_at - interval '1 hour', claimed_at = claimed_at - interval '1 hour' WHERE id = $1")
            .bind(job)
            .execute(pool)
            .await
            .unwrap();
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn claimed_job_fails_once(pool: PgPool) {
        let job = enqueue(&pool).await;
        claim_as_judge(&pool).await;
        assert!(fail_as_judge(&pool, job).await);
        assert!(!fail_as_judge(&pool, job).await);
        assert_eq!(state_and_result(&pool, job).await, ("failed".into(), None));
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn waiting_job_cannot_fail(pool: PgPool) {
        let job = enqueue(&pool).await;
        assert!(!fail_as_judge(&pool, job).await);
        assert_eq!(state_and_result(&pool, job).await, ("waiting".into(), None));
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn only_the_judge_can_fail(pool: PgPool) {
        let job = enqueue(&pool).await;
        claim_as_judge(&pool).await;
        let mut transaction = begin_as(&pool, AS_API).await;
        assert_denied(&fail(&mut transaction, job).await.unwrap_err());
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn leased_job_is_not_reclaimed(pool: PgPool) {
        let job = enqueue(&pool).await;
        assert_eq!(claim_as_judge(&pool).await, Some(job));
        assert_eq!(claim_as_judge(&pool).await, None);
        assert_eq!(state_and_result(&pool, job).await, ("running".into(), None));
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn abandoned_job_is_reclaimed_first(pool: PgPool) {
        let abandoned = enqueue(&pool).await;
        claim_as_judge(&pool).await;
        let newer = enqueue(&pool).await;
        expire(&pool, abandoned).await;
        assert_eq!(claim_as_judge(&pool).await, Some(abandoned));
        assert_eq!(claim_as_judge(&pool).await, Some(newer));
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn job_abandoned_three_times_fails(pool: PgPool) {
        let job = enqueue(&pool).await;
        for _ in 0..3 {
            assert_eq!(claim_as_judge(&pool).await, Some(job));
            expire(&pool, job).await;
        }
        assert_eq!(claim_as_judge(&pool).await, None);
        assert_eq!(state_and_result(&pool, job).await, ("failed".into(), None));
    }
    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn running_job_reports_its_cases(pool: PgPool) {
        let job = enqueue(&pool).await;
        claim_as_judge(&pool).await;
        assert!(report_as_judge(&pool, job, &passed_at(0)).await);
        assert!(report_as_judge(&pool, job, &passed_at(1)).await);
        assert_eq!(
            reported_cases(&pool, job).await,
            [(0, "passed".to_string()), (1, "passed".to_string())]
        );
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn case_reported_twice_keeps_last(pool: PgPool) {
        let job = enqueue(&pool).await;
        claim_as_judge(&pool).await;
        let wrong = CaseReport {
            verdict: Verdict::WrongAnswer,
            ..passed_at(0)
        };
        assert!(report_as_judge(&pool, job, &wrong).await);
        assert!(report_as_judge(&pool, job, &passed_at(0)).await);
        assert_eq!(
            reported_cases(&pool, job).await,
            [(0, "passed".to_string())]
        );
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn only_a_running_job_reports(pool: PgPool) {
        let job = enqueue(&pool).await;
        assert!(!report_as_judge(&pool, job, &passed_at(0)).await);

        claim_as_judge(&pool).await;
        assert!(finish_as_judge(&pool, job, &[Verdict::Passed]).await);
        assert!(!report_as_judge(&pool, job, &passed_at(0)).await);

        assert_eq!(reported_cases(&pool, job).await, []);
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn only_the_judge_can_report(pool: PgPool) {
        let job = enqueue(&pool).await;
        claim_as_judge(&pool).await;
        let mut transaction = begin_as(&pool, AS_API).await;
        let error = report_case(&mut transaction, job, &passed_at(0))
            .await
            .unwrap_err();
        assert_denied(&error);
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn judge_cannot_touch_case_rows(pool: PgPool) {
        let mut transaction = begin_as(&pool, AS_JUDGE).await;
        let error = sqlx::query("SELECT actual FROM job_cases")
            .execute(&mut *transaction)
            .await
            .unwrap_err();
        assert_denied(&error);
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn reclaimed_job_loses_its_cases(pool: PgPool) {
        let job = enqueue(&pool).await;
        claim_as_judge(&pool).await;
        report_as_judge(&pool, job, &passed_at(0)).await;
        expire(&pool, job).await;
        assert_eq!(claim_as_judge(&pool).await, Some(job));
        assert_eq!(reported_cases(&pool, job).await, []);
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn claiming_keeps_other_jobs_cases(pool: PgPool) {
        let job = enqueue(&pool).await;
        claim_as_judge(&pool).await;
        report_as_judge(&pool, job, &passed_at(0)).await;
        claim_as_judge(&pool).await;
        assert_eq!(
            reported_cases(&pool, job).await,
            [(0, "passed".to_string())]
        );
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn a_case_is_deleted_with_its_job(pool: PgPool) {
        let job = enqueue(&pool).await;
        claim_as_judge(&pool).await;
        report_as_judge(&pool, job, &passed_at(0)).await;
        sqlx::query("DELETE FROM jobs WHERE id = $1")
            .bind(job)
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(reported_cases(&pool, job).await, []);
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn reclaimed_job_can_still_finish(pool: PgPool) {
        let job = enqueue(&pool).await;
        claim_as_judge(&pool).await;
        expire(&pool, job).await;
        claim_as_judge(&pool).await;
        assert!(finish_as_judge(&pool, job, &[Verdict::Passed]).await);
        assert!(!finish_as_judge(&pool, job, &[Verdict::Passed]).await);
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn a_closed_draft_becomes_one_final_job(pool: PgPool) {
        schedule_sum(&pool, "-1 minute").await;
        save_draft(&pool, "etudiant1", "print(1)").await;
        save_draft(&pool, "etudiant2", "").await;
        assert_eq!(enqueue_finals_as_judge(&pool).await, 1);
        assert_eq!(enqueue_finals_as_judge(&pool).await, 0);
        let on_time: bool = sqlx::query_scalar(
            "SELECT jobs.enqueued_at = drafts.saved_at FROM jobs JOIN drafts USING (author)",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(on_time);
        assert_eq!(claimed_kind(&pool).await, JobKind::Final);
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn an_open_draft_is_not_final(pool: PgPool) {
        schedule_sum(&pool, "1 hour").await;
        save_draft(&pool, "etudiant1", "print(1)").await;
        assert_eq!(enqueue_finals_as_judge(&pool).await, 0);
    }
}
