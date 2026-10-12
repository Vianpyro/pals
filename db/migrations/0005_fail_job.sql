-- Judge fails only through this function: no direct access to jobs (ADR-0001)

CREATE FUNCTION fail_job(job uuid)
RETURNS boolean
LANGUAGE sql SECURITY DEFINER SET search_path = pg_catalog, pg_temp
BEGIN ATOMIC
    WITH failed AS (
        UPDATE public.jobs
        SET state = 'failed', finished_at = now()
        WHERE jobs.id = job AND jobs.state = 'running'
        RETURNING 1
    )
    SELECT count(*) = 1 FROM failed;
END;

REVOKE EXECUTE ON FUNCTION fail_job(uuid) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION fail_job(uuid) TO judge;
