-- When an activity opens: entered in the platform, never in the content (ADR-0020)
-- The offering's row only for now; group rows and extensions come with groups

CREATE TABLE schedules (
    course identifier NOT NULL,
    activity identifier NOT NULL,
    opens_at timestamptz NOT NULL,
    closes_at timestamptz NULL,

    PRIMARY KEY (course, activity),
    CONSTRAINT schedules_closes_after_opening CHECK (closes_at > opens_at)
);

-- API: reads the schedule on every access to an activity
GRANT SELECT ON schedules TO api;
