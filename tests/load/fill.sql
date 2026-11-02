-- Fills the queue at once, to measure how many jobs per minute N judges sustain
-- From the repository root: psql -v run=cap-4 -v exercise=sum -v jobs=300 -f tests/load/fill.sql "$DATABASE_URL"

\set ON_ERROR_STOP on
\set passed `cat tests/load/corpus/passed.py`
\set wrong `cat tests/load/corpus/wrong.py`
\set crashed `cat tests/load/corpus/crashed.py`
\set timed_out `cat tests/load/corpus/timed-out.py`
\set memory `cat tests/load/corpus/memory.py`
\set flood `cat tests/load/corpus/flood.py`

-- Same mix as MIX in submissions.js
INSERT INTO jobs (contract, course, exercise, language_pack, source)
SELECT 1, :'run', :'exercise', 'python', CASE
    WHEN draw < 0.40 THEN :'passed'
    WHEN draw < 0.75 THEN :'wrong'
    WHEN draw < 0.90 THEN :'crashed'
    WHEN draw < 0.97 THEN :'timed_out'
    WHEN draw < 0.99 THEN :'memory'
    ELSE :'flood'
END
FROM (SELECT random() AS draw FROM generate_series(1, :jobs)) AS draws;
