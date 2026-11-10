-- A choice item is graded only by a final job: a check would reveal its key (ADR-0015)

CREATE FUNCTION finish_choice(job uuid, score smallint)
RETURNS boolean
LANGUAGE sql SECURITY DEFINER SET search_path = pg_catalog, pg_temp
BEGIN ATOMIC
    WITH finished AS (
        UPDATE public.jobs
        SET state = 'done', finished_at = now(), result = jsonb_build_object('score', score)
        WHERE jobs.id = job AND jobs.state = 'running' AND jobs.kind = 'final'
            AND score BETWEEN 0 AND 100
        RETURNING 1
    )
    SELECT count(*) = 1 FROM finished;
END;

REVOKE EXECUTE ON FUNCTION finish_choice(uuid, smallint) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION finish_choice(uuid, smallint) TO judge;
