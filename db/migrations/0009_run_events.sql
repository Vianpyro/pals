-- A run's author is told when it is created or changes state: the API listens and forwards it
-- over Server-Sent Events (ADR-0008, ADR-0030).
-- The payload is the run, then its author, so a stream skips the other students' runs without a query.

CREATE FUNCTION notify_run_changed()
RETURNS trigger
LANGUAGE plpgsql SET search_path = pg_catalog, pg_temp
AS $$
BEGIN
    PERFORM pg_notify('runs', NEW.id::text || ' ' || NEW.author);
    RETURN NULL;
END
$$;

CREATE TRIGGER run_changed
AFTER INSERT OR UPDATE OF state ON jobs
FOR EACH ROW EXECUTE FUNCTION notify_run_changed();

CREATE FUNCTION notify_case_ended()
RETURNS trigger
LANGUAGE plpgsql SET search_path = pg_catalog, pg_temp
AS $$
BEGIN
    -- Author last so splitting stays correct even if it held a space
    PERFORM pg_notify('cases', NEW.job::text || ' ' || NEW.position || ' ' || (SELECT author FROM public.jobs WHERE id = NEW.job));
    RETURN NULL;
END
$$;

CREATE TRIGGER case_ended
AFTER INSERT OR UPDATE ON job_cases
FOR EACH ROW EXECUTE FUNCTION notify_case_ended();
