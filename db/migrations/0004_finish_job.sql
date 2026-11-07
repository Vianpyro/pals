-- Judge finishes only through this function: no direct access to jobs (ADR-0001)

CREATE FUNCTION finish_job(job uuid, verdicts case_verdict[])
RETURNS boolean
LANGUAGE sql SECURITY DEFINER SET search_path = pg_catalog, pg_temp
BEGIN ATOMIC
    WITH finished AS (
        UPDATE public.jobs
        SET state = 'done', finished_at = now(), result = jsonb_build_object('cases', to_jsonb(verdicts))
        WHERE jobs.id = job AND jobs.state = 'running' AND cardinality(verdicts) > 0
        RETURNING 1
    )
    SELECT count(*) = 1 FROM finished;
END;

REVOKE EXECUTE ON FUNCTION finish_job(uuid, case_verdict[]) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION finish_job(uuid, case_verdict[]) TO judge;

CREATE FUNCTION report_case(job uuid, case_position integer, verdict case_verdict, actual text, truncated boolean)
RETURNS boolean
LANGUAGE sql SECURITY DEFINER SET search_path = pg_catalog, pg_temp
BEGIN ATOMIC
    WITH reported AS (
        INSERT INTO public.job_cases (job, position, verdict, actual, truncated)
        SELECT jobs.id, case_position, report_case.verdict, report_case.actual, report_case.truncated
        FROM public.jobs
        WHERE jobs.id = report_case.job AND jobs.state = 'running'
        ON CONFLICT ON CONSTRAINT job_cases_pkey DO UPDATE
        SET verdict = EXCLUDED.verdict, actual = EXCLUDED.actual, truncated = EXCLUDED.truncated
        RETURNING 1
    )
    SELECT count(*) = 1 FROM reported;
END;

REVOKE EXECUTE ON FUNCTION report_case(uuid, integer, case_verdict, text, boolean) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION report_case(uuid, integer, case_verdict, text, boolean) TO judge;
