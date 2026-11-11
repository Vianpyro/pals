-- Who holds which role in an offering (ADR-0012)
-- The directory gives identity, never roles (ADR-0017); admin is global and lives elsewhere

CREATE TYPE offering_role AS ENUM (
    'student',
    'ta',
    'instructor',
    'coordinator'
);

CREATE TABLE offering_roles (
    offering identifier NOT NULL REFERENCES offerings ON DELETE CASCADE,
    account text NOT NULL CHECK (account <> ''),
    role offering_role NOT NULL,

    PRIMARY KEY (offering, account)
);

GRANT SELECT ON offering_roles TO api;
-- API: maps an offering to its content directory
GRANT SELECT ON offerings TO api;
