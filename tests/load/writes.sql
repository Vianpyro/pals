-- Draft autosaves until the API writes them, at ≈ 0.4 writes/s per student (architecture: Target load)
-- Once: psql -c "CREATE TABLE load_drafts (student integer, exercise integer, source text NOT NULL, saved_at timestamptz NOT NULL, PRIMARY KEY (student, exercise))"
-- pgbench -n -f tests/load/writes.sql -D students=250 -R 100 -c 10 -T 2400 "$DATABASE_URL"

\set student random(1, :students)
\set exercise random(1, 5)
-- shortcut: 2 KiB per draft, take the median source size from the TCH009 CTester logs
INSERT INTO load_drafts VALUES (:student, :exercise, repeat('x', 2048), now())
ON CONFLICT (student, exercise) DO UPDATE SET source = excluded.source, saved_at = excluded.saved_at;
