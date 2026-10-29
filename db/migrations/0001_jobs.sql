-- Submission queue (ADR-0001)

CREATE TYPE job_state as ENUM (
    'waiting',
    'running',
    'done',
    'replaced',
    'failed'
);

CREATE DOMAIN identifier AS text CHECK (value ~ '^[a-z0-9-]+$');

CREATE TABLE jobs (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    contract integer NOT NULL CHECK (contract > 0),
    course identifier NOT NULL,
    exercise identifier NOT NULL,
    language_pack identifier NOT NULL, -- (ADR-0013)
    source text NOT NULL CHECK (octet_length(source) BETWEEN 1 and 1024*1024),
    state job_state NOT NULL DEFAULT 'waiting',
    enqueued_at timestamptz NOT NULL DEFAULT now(),
    claimed_at timestamptz,
    claims integer NOT NULL DEFAULT 0 CHECK (claims >= 0),
    finished_at timestamptz,
    result jsonb,

    CONSTRAINT jobs_claimed_when_run CHECK ((claimed_at IS NOT NULL) = (state IN ('running', 'done', 'failed'))),
    CONSTRAINT jobs_finished_when_over CHECK ((finished_at IS NOT NULL) = (state IN ('done', 'replaced', 'failed'))),
    CONSTRAINT jobs_result_when_done CHECK ((result IS NOT NULL) = (state = 'done')),
    CONSTRAINT jobs_claimed_after_enqueued CHECK (claimed_at >= enqueued_at),
    CONSTRAINT jobs_finished_after_start CHECK (finished_at >= coalesce(claimed_at, enqueued_at))
);

CREATE INDEX jobs_waiting_idx ON jobs (enqueued_at, id) WHERE state = 'waiting';
CREATE INDEX jobs_running_idx ON jobs (claimed_at) WHERE state = 'running';
