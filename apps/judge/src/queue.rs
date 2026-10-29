//! Jobs claimed from the PostgreSQL queue (ADR-0001)

use sqlx::{FromRow, PgConnection, types::Uuid};

use crate::Verdict;

#[derive(Debug, FromRow)]
pub struct Job {
    pub id: Uuid,
    pub contract: i32,
    pub course: String,
    pub exercise: String,
    pub language_pack: String,
    pub source: String,
}

pub async fn claim(connection: &mut PgConnection) -> sqlx::Result<Option<Job>> {
    sqlx::query_as("SELECT id, contract, course, exercise, language_pack, source FROM claim_job()")
        .fetch_optional(connection)
        .await
}

pub async fn finish(
    connection: &mut PgConnection,
    job: Uuid,
    verdicts: &[Verdict],
) -> sqlx::Result<bool> {
    sqlx::query_scalar("SELECT finish_job($1, $2)")
        .bind(job)
        .bind(verdicts)
        .fetch_one(connection)
        .await
}

pub async fn fail(connection: &mut PgConnection, job: Uuid) -> sqlx::Result<bool> {
    sqlx::query_scalar("SELECT fail_job($1)")
        .bind(job)
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
            "INSERT INTO jobs (contract, course, exercise, language_pack, source) VALUES (1, 'log200', 'sum', 'python', 'print(0)') RETURNING id",
        )
        .fetch_one(pool)
        .await
        .unwrap()
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
    async fn jobs_are_claimed_oldest_first_and_once(pool: PgPool) {
        let first = enqueue(&pool).await;
        let second = enqueue(&pool).await;
        assert_eq!(claim_as_judge(&pool).await, Some(first));
        assert_eq!(claim_as_judge(&pool).await, Some(second));
        assert_eq!(claim_as_judge(&pool).await, None);
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
    async fn judge_has_no_direct_access_to_jobs(pool: PgPool) {
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
    async fn running_job_within_its_lease_is_not_reclaimed(pool: PgPool) {
        let job = enqueue(&pool).await;
        assert_eq!(claim_as_judge(&pool).await, Some(job));
        assert_eq!(claim_as_judge(&pool).await, None);
        assert_eq!(state_and_result(&pool, job).await, ("running".into(), None));
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn abandoned_job_is_reclaimed_before_newer_jobs(pool: PgPool) {
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
    async fn first_judge_can_still_finish_a_reclaimed_job(pool: PgPool) {
        let job = enqueue(&pool).await;
        claim_as_judge(&pool).await;
        expire(&pool, job).await;
        claim_as_judge(&pool).await;
        assert!(finish_as_judge(&pool, job, &[Verdict::Passed]).await);
        assert!(!finish_as_judge(&pool, job, &[Verdict::Passed]).await);
    }
}
