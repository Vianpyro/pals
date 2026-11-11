-- At the close, each saved draft becomes the student's final submission (ADR-0040)

CREATE UNIQUE INDEX jobs_one_final_idx ON jobs (author, offering, exercise) WHERE kind = 'final';

CREATE FUNCTION enqueue_finals()
RETURNS integer
LANGUAGE sql SECURITY DEFINER SET search_path = pg_catalog, pg_temp
BEGIN ATOMIC
    WITH queued AS (
        -- Saved before the close: the judge accepts it as submitted in time
        INSERT INTO public.jobs (contract, kind, offering, exercise, language_pack, author, source, enqueued_at)
        SELECT 1, 'final', drafts.offering, drafts.exercise, drafts.language_pack, drafts.author, drafts.source, drafts.saved_at
        FROM public.drafts
        JOIN public.schedules ON schedules.offering = drafts.offering AND schedules.activity = drafts.exercise
        WHERE schedules.closes_at <= now() AND drafts.source <> ''
        ON CONFLICT (author, offering, exercise) WHERE kind = 'final' DO NOTHING
        RETURNING 1
    )
    SELECT count(*)::integer FROM queued;
END;

REVOKE EXECUTE ON FUNCTION enqueue_finals() FROM PUBLIC;
GRANT EXECUTE ON FUNCTION enqueue_finals() TO judge;
