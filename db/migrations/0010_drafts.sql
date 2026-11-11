-- The server copy of each student's draft of an exercise (ADR-0038)

CREATE TABLE drafts (
    author text NOT NULL CHECK (author <> ''),
    offering identifier NOT NULL REFERENCES offerings ON DELETE CASCADE,
    exercise identifier NOT NULL,
    language_pack identifier NOT NULL,
    revision bigint NOT NULL CHECK (revision > 0),
    source text NOT NULL CHECK (octet_length(source) <= 1024 * 1024),
    saved_at timestamptz NOT NULL DEFAULT now(),

    PRIMARY KEY (author, offering, exercise)
);

GRANT SELECT, INSERT, UPDATE (revision, source, saved_at, language_pack) ON drafts TO api;
