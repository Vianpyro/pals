-- Judge claims only through this function: no direct access to jobs (ADR-0001)
-- Abandoned jobs are reclaimed first (ADR-0010)

CREATE FUNCTION claim_job()
RETURNS TABLE (
    id uuid,
    contract integer,
    kind job_kind,
    offering identifier,
    course identifier,
    exercise identifier,
    language_pack identifier,
    source text,
    enqueued_at timestamptz
)
LANGUAGE sql SECURITY DEFINER SET search_path = pg_catalog, pg_temp
BEGIN ATOMIC
    UPDATE public.jobs SET state = 'failed', finished_at = now()
    WHERE jobs.state = 'running' AND jobs.claimed_at < now() - interval '2 minutes' AND jobs.claims >= 3;

    -- A reclaimed job reports its cases again (ADR-0030)
    DELETE FROM public.job_cases USING public.jobs
    WHERE job_cases.job = jobs.id AND jobs.state = 'running' AND jobs.claimed_at < now() - interval '2 minutes';

    UPDATE public.jobs SET state = 'waiting', claimed_at = NULL
    WHERE jobs.state = 'running' AND jobs.claimed_at < now() - interval '2 minutes';

    UPDATE public.jobs SET state = 'running', claimed_at = now(), claims = jobs.claims + 1
    FROM public.offerings
    WHERE offerings.id = jobs.offering AND jobs.id = (
        SELECT j.id FROM public.jobs j
        WHERE j.state = 'waiting'
        ORDER BY j.enqueued_at, j.id LIMIT 1 FOR UPDATE SKIP LOCKED
    )
    RETURNING jobs.id, jobs.contract, jobs.kind, jobs.offering, offerings.course, jobs.exercise, jobs.language_pack, jobs.source, jobs.enqueued_at;
END;

REVOKE EXECUTE ON FUNCTION claim_job() FROM PUBLIC;
GRANT EXECUTE ON FUNCTION claim_job() TO judge;
