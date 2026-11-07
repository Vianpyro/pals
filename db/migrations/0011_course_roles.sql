-- Who holds which role in a course (ADR-0012)
-- The directory gives identity, never roles (ADR-0017)
-- Keyed by course ultil offerings exist; admin is global and lives elsewhere

CREATE TYPE course_role AS ENUM (
    'student',
    'ta',
    'instructor',
    'coordinator'
);

CREATE TABLE course_roles (
    course identifier NOT NULL,
    account text NOT NULL CHECK (account <> ''),
    role course_role NOT NULL,

    PRIMARY KEY (course, account)
);

GRANT SELECT ON course_roles TO api;
