#!/usr/bin/env bash
# Starts the demo behind the page at localhost:80: the example course published, its exercise open,
# the API and the judge running. Safe to rerun; Ctrl+C stops both.
set -euo pipefail
cd "$(dirname "$0")/.."

demo=/tmp/demo
bin=${CARGO_TARGET_DIR:-target}/debug
# The judge's Python pack image (packs/languages/python/pack.toml)
image=python:3.15-alpine

# The inner dockerd starts alongside this script
for _ in $(seq 30); do
  docker info >/dev/null 2>&1 && break
  sleep 1
done
# Otherwise the first job pulls it within the judge's time limit
docker image inspect "$image" >/dev/null 2>&1 || docker pull -q "$image" >/dev/null

cargo build -p api -p judge -p publisher
# The page nginx serves at localhost:80
npm run build -w apps/web

rm -rf "$demo"
mkdir -p "$demo/content" "$demo/courses/log200"
ln -s "$PWD/examples/content" "$demo/content/log200"
"$bin/publisher" examples/content "$demo/courses/log200" >/dev/null
"$bin/api" migrate
# DO UPDATE, not DO NOTHING: a row left closed would hide the exercise
psql -q -v ON_ERROR_STOP=1 "$DATABASE_URL" -c "
  INSERT INTO schedules VALUES ('log200', 'sum', now() - interval '1 hour', NULL)
  ON CONFLICT (course, activity) DO UPDATE SET opens_at = excluded.opens_at, closes_at = NULL"

# Whichever stops first, the other goes with it; traps cleared first, or kill 0 would run them again
trap 'trap - EXIT INT TERM HUP; kill 0' EXIT INT TERM HUP
COURSES="$demo/courses" "$bin/api" &
PACKS="$PWD/packs/languages" CONTENT="$demo/content" COURSES="$demo/courses" "$bin/judge" &
echo "Démo prête : onglet Ports → 80 (etudiant1 / log791-dev)"
wait -n
