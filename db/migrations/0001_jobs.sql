-- Submission queue (ADR-0001)

CREATE TYPE job_state as ENUM (
    'waiting',
    'running',
    'done',
    'replaced',
    'failed'
);

CREATE TYPE job_kind AS ENUM (
    'precheck',
    'check',
    'final'
);

CREATE TYPE case_verdict AS ENUM (
    'passed',
    'wrong-answer',
    'crashed',
    'timed-out'
);

CREATE TABLE jobs (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    contract integer NOT NULL CHECK (contract > 0),
    kind job_kind NOT NULL DEFAULT 'check',
    offering identifier NOT NULL REFERENCES offerings ON DELETE CASCADE,
    exercise identifier NOT NULL,
    language_pack identifier NOT NULL, -- (ADR-0013)
    author text NOT NULL CHECK (author <> ''), -- (ADR-0017)
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

CREATE TABLE job_cases (
    job uuid NOT NULL REFERENCES jobs ON DELETE CASCADE,
    position integer NOT NULL CHECK (position >= 0),
    verdict case_verdict NOT NULL,
    actual text NOT NULL CHECK (octet_length(actual) <= 64 * 1024), -- (ADR-0042)
    truncated boolean NOT NULL,

    PRIMARY KEY (job, position)
);

CREATE INDEX jobs_waiting_idx ON jobs (enqueued_at, id) WHERE state = 'waiting';
CREATE INDEX jobs_running_idx ON jobs (claimed_at) WHERE state = 'running';
