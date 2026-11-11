-- A course given in one term: everything a term holds is keyed by its offering (ADR-0012)

CREATE DOMAIN identifier AS text CHECK (value ~ '^[a-z0-9-]+$');

CREATE TABLE offerings (
    id identifier PRIMARY KEY,
    course identifier NOT NULL, -- names the content directory courses/<course>
    term identifier NOT NULL,

    UNIQUE (course, term)
);
