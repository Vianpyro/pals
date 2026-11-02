-- Queue delays, failures and throughput of one load run, from the database clock
-- psql -v run=load-250 -f tests/load/report.sql "$DATABASE_URL"

\set ON_ERROR_STOP on

-- Jobs that are not done, or ran twice: a reclaim means a job outlived its lease
SELECT
    count(*) AS jobs,
    count(*) FILTER (WHERE state = 'failed') AS failed,
    count(*) FILTER (WHERE state IN ('waiting', 'running')) AS unfinished,
    count(*) FILTER (WHERE claims > 1) AS reclaimed
FROM jobs
WHERE course = :'run';

-- claimed_at is the last claim: the queue delay of a reclaimed job includes its first run
SELECT
    step,
    count(*) AS jobs,
    percentile_cont(ARRAY[0.5, 0.95, 0.99]) WITHIN GROUP (ORDER BY seconds) AS "p50, p95, p99 (s)",
    max(seconds) AS "max (s)"
FROM jobs,
    LATERAL (VALUES
        ('1 queue', claimed_at - enqueued_at),
        ('2 run', finished_at - claimed_at),
        ('3 total', finished_at - enqueued_at)
    ) AS steps (step, delay),
    LATERAL (SELECT extract(epoch FROM delay)) AS delays (seconds)
WHERE course = :'run' AND state = 'done'
GROUP BY step
ORDER BY step;

-- Verdicts actually produced, to check the corpus mix ran as intended
SELECT verdict, count(*) AS cases
FROM jobs, jsonb_array_elements_text(result -> 'cases') AS verdict
WHERE course = :'run'
GROUP BY verdict
ORDER BY cases DESC;

-- Arrivals against completions: the judges saturate once finished stays below enqueued
SELECT
    minute,
    count(*) FILTER (WHERE event = 'enqueued') AS enqueued,
    count(*) FILTER (WHERE event = 'finished') AS finished
FROM jobs,
    LATERAL (VALUES ('enqueued', enqueued_at), ('finished', finished_at)) AS events (event, at),
    LATERAL (SELECT date_trunc('minute', at)) AS minutes (minute)
WHERE course = :'run' AND at IS NOT NULL
GROUP BY minute
ORDER BY minute;
