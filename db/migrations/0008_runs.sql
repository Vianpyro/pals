-- A run as its author sees it in their history: whether every case passed (ADR-0030)
-- Per-case detail of visible tests is read elsewhere; hidden ones never show which case failed

CREATE VIEW runs AS
SELECT id, author, course, exercise, state, enqueued_at, (result -> 'cases') <@ '["passed"]' AS passed
FROM jobs;

GRANT SELECT ON runs TO api;
