-- Session opened by an LDAP sign-in (ADR-0017)
-- Only the token's hash is stored: reading this table, or a backup of it, opens no session

CREATE TABLE sessions (
    token_hash bytea PRIMARY KEY CHECK (octet_length(token_hash) = 32),
    account text NOT NULL CHECK (account <> ''),
    opened_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz NOT NULL DEFAULT now() + interval '12 hours',

    CONSTRAINT sessions_expire_after_opening CHECK (expires_at > opened_at)
);

-- API: opens, checks and closes sessions
GRANT SELECT, INSERT, DELETE ON sessions TO api;
