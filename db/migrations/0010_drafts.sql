-- The server copy of each student's draft of an exercise (ADR-0038)

CREATE TABLE drafts (
    author text NOT NULL CHECK (author <> ''),
    course identifier NOT NULL,
    exercise identifier NOT NULL,
    revision bigint NOT NULL CHECK (revision > 0),
    source text NOT NULL CHECK (octet_length(source) <= 1024 * 1024),
    saved_at timestamptz NOT NULL DEFAULT now(),

    PRIMARY KEY (author, course, exercise)
);

GRANT SELECT, INSERT, UPDATE (revision, source, saved_at) ON drafts TO api;
