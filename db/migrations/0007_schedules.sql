-- When an activity opens: entered in the platform, never in the content (ADR-0020)

-- A student is in exactly one group per offering
CREATE TABLE groups (
    offering identifier NOT NULL REFERENCES offerings ON DELETE CASCADE,
    id identifier NOT NULL,

    PRIMARY KEY (offering, id)
);

CREATE TABLE group_members (
    offering identifier NOT NULL,
    account text NOT NULL CHECK (account <> ''),
    group_id identifier NOT NULL,

    PRIMARY KEY (offering, account),
    FOREIGN KEY (offering, group_id) REFERENCES groups ON DELETE CASCADE
);

CREATE TABLE schedules (
    offering identifier NOT NULL REFERENCES offerings ON DELETE CASCADE,
    activity identifier NOT NULL,
    group_id identifier NULL,
    account text NULL CHECK (account <> ''),
    opens_at timestamptz NOT NULL,
    closes_at timestamptz NULL,

    UNIQUE NULLS NOT DISTINCT (offering, activity, group_id, account),
    FOREIGN KEY (offering, group_id) REFERENCES groups ON DELETE CASCADE,
    CONSTRAINT schedules_group_or_account CHECK (group_id IS NULL OR account IS NULL),
    CONSTRAINT schedules_closes_after_opening CHECK (closes_at > opens_at)
);

-- API and Judge: read the schedule on every access to an activity
GRANT SELECT ON groups, group_members, schedules TO api, judge;
