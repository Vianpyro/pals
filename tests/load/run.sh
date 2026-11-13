#!/usr/bin/env bash
# One measure of the exam load, written as JSON for tests/load/sizing.js. Inside the dev container.
# Usage: bash tests/load/run.sh <students> <judges> <steady|burst> <pack> <seconds> <output.json>
#
# Touches nothing of the development database: a throwaway PostgreSQL runs in the inner dockerd,
# with sessions whose tokens are guessable, so this must never point at a real database.
set -euo pipefail
cd "$(dirname "$0")/../.."

students=$1 judges=$2 mode=$3 pack=$4 seconds=$5 output=$6
case "$mode" in steady | burst) ;; *) echo "mode: steady or burst, not $mode" >&2; exit 2 ;; esac
for number in "$students" "$judges" "$seconds"; do
  [[ $number =~ ^[1-9][0-9]*$ ]] || { echo "not a positive number: $number" >&2; exit 2; }
done

source_file=$(echo "packs/languages/$pack"/conformance/passed/sum.*)
[ -f "$source_file" ] || { echo "no reference solution for the pack $pack" >&2; exit 2; }
if curl -s -m 2 -o /dev/null http://localhost:8080/health; then
  echo "port 8080 is taken: stop the demo first" >&2
  exit 1
fi

port=55432
database="postgresql://log791:log791@localhost:$port/log791"
image=$(sed -n 's/^ *image: \(postgres:[^[:space:]]*\).*/\1/p' .devcontainer/compose.yml | head -1)
pack_image=$(sed -n 's/^image = "\(.*\)"$/\1/p' "packs/languages/$pack/pack.toml")
bin=${CARGO_TARGET_DIR:-target}/release
work=$(mktemp -d)
sandboxes=/sys/fs/cgroup/docker
pids=()

stop_all() {
  trap - EXIT INT TERM HUP
  [ ${#pids[@]} -eq 0 ] || kill "${pids[@]}" 2>/dev/null || true
  docker rm --force load-postgres load-keepalive >/dev/null 2>&1 || true
  rm -rf "$work"
}
trap stop_all EXIT INT TERM HUP

sql() { psql -qAt -v ON_ERROR_STOP=1 "$database" -c "$1"; }
usage() { awk '/^usage_usec/ { printf "%.3f", $2 / 1000000 }' "$1/cpu.stat"; }
mib() { awk '{ printf "%d", $1 / 1048576 }' "$1"; }
status_mib() { awk -v key="$2:" '$1 == key { printf "%d", $2 / 1024 }' "/proc/$1/status"; }
# User and system time, with the docker CLI runs the process has waited for
cpu_of() { awk '{ print ($14 + $15 + $16 + $17) / 100 }' "/proc/$1/stat"; }

for _ in $(seq 30); do
  docker info >/dev/null 2>&1 && break
  sleep 1
done
docker image inspect "$image" >/dev/null 2>&1 || docker pull -q "$image" >/dev/null
docker image inspect "$pack_image" >/dev/null 2>&1 || docker pull -q "$pack_image" >/dev/null
SQLX_OFFLINE=true cargo build --release --locked -p api -p judge -p publisher

# Docker removes its parent cgroup with the last container, and with it the counters of the
# sandboxes that have ended: this one holds it for the whole measure.
docker rm --force load-postgres load-keepalive >/dev/null 2>&1 || true
docker run -d --name load-keepalive --entrypoint sleep "$image" infinity >/dev/null
# Outside that parent, so the sandboxes' counters do not include PostgreSQL
docker run -d --name load-postgres --network host --cgroup-parent /load \
  -e POSTGRES_USER=log791 -e POSTGRES_PASSWORD=log791 -e POSTGRES_DB=log791 \
  "$image" -c "port=$port" >/dev/null
postgres=/sys/fs/cgroup/load/$(docker inspect -f '{{.Id}}' load-postgres)
for _ in $(seq 60); do
  psql -qAt "$database" -c 'SELECT 1' >/dev/null 2>&1 && break
  sleep 1
done

mkdir -p "$work/content" "$work/courses/log200"
ln -s "$PWD/examples/content" "$work/content/log200"
"$bin/publisher" examples/content "$work/courses/log200" >/dev/null
DATABASE_URL=$database "$bin/api" migrate
sql "INSERT INTO offerings VALUES ('log200-a26', 'log200', 'a26')"
sql "INSERT INTO schedules (offering, activity, opens_at, closes_at) VALUES ('log200-a26', 'sum', now() - interval '1 hour', NULL)"
# The offering opens only to the accounts on its roster (ADR-0012); not seq -f %g, which writes 1e+06
{ echo account,role; seq "$students" | sed 's/.*/student&,student/'; } |
  DATABASE_URL=$database "$bin/api" enroll log200-a26 >/dev/null
sql "INSERT INTO sessions (token_hash, account)
     SELECT sha256(('load-' || i)::bytea), 'student' || i FROM generate_series(1, $students) i"

DATABASE_URL=$database COURSES="$work/courses" "$bin/api" &
api=$!
pids+=("$api")
for _ in $(seq 30); do
  curl -sf -m 2 http://localhost:8080/health >/dev/null && break
  sleep 1
done
curl -sf -m 2 http://localhost:8080/health >/dev/null || { echo "the API never answered" >&2; exit 1; }

api_idle=$(status_mib "$api" VmRSS)
api_cpu_idle=$(cpu_of "$api")
postgres_idle=$(mib "$postgres/memory.current")
postgres_cpu_idle=$(usage "$postgres")
database_idle=$(sql "SELECT pg_database_size('log791')")
sandboxes_cpu_idle=$(usage "$sandboxes")

judge_pids=()
for _ in $(seq "$judges"); do
  DATABASE_URL=$database PACKS="$PWD/packs/languages" CONTENT="$work/content" COURSES="$work/courses" "$bin/judge" &
  judge_pids+=($!)
  pids+=($!)
done

# What each sandbox costs outside its own cgroup: its docker CLI, containerd shim and gofer.
# Resident sizes count shared pages each time, which only overstates it.
# The sandboxes' memory is sampled too: their parent cgroup can outlive a measure, and so its peak.
(
  while :; do
    outside=$(ps -eo rss=,args= | awk '$2 ~ /(^|\/)docker$/ && $3 == "run" || /containerd-shim|runsc-gofer/ { kib += $1 } END { print kib + 0 }')
    echo "$outside $(cat "$sandboxes/memory.current")"
    sleep 0.5
  done
) >"$work/samples" &
pids+=($!)

begin=$(date +%s.%N)
http=$(node tests/load/exam.js http://localhost:8080 "$students" "$mode" "$seconds" "$pack" "$source_file")

# Jobs a student submitted just before the end still count
for _ in $(seq 1800); do
  [ "$(sql "SELECT count(*) FROM jobs WHERE state IN ('waiting', 'running')")" = 0 ] && break
  sleep 1
done
pending=$(sql "SELECT count(*) FROM jobs WHERE state IN ('waiting', 'running')")
[ "$pending" = 0 ] || { echo "$pending jobs never finished" >&2; exit 1; }
end=$(date +%s.%N)

judge_each=0
judges_cpu=0
for judge in "${judge_pids[@]}"; do
  kill -0 "$judge" 2>/dev/null || { echo "a judge stopped during the measure" >&2; exit 1; }
  judge_each=$(awk -v a="$judge_each" -v b="$(status_mib "$judge" VmHWM)" 'BEGIN { print (b > a) ? b : a }')
  judges_cpu=$(awk -v a="$judges_cpu" -v b="$(cpu_of "$judge")" 'BEGIN { print a + b }')
done
overhead_each=$(awk -v c="$judges" '$1 > peak { peak = $1 } END { printf "%d", peak / 1024 / c }' "$work/samples")
sandboxes_peak=$(awk '$2 > peak { peak = $2 } END { printf "%d", peak / 1048576 }' "$work/samples")

# All the numbers above are this script's own: none comes from the load's responses
sql "
  SELECT json_build_object(
    'students', $students, 'judges', $judges, 'mode', '$mode', 'pack', '$pack', 'host_cores', $(nproc),
    'jobs', (
      SELECT json_build_object(
        'done', count(*) FILTER (WHERE state = 'done'),
        'replaced', count(*) FILTER (WHERE state = 'replaced'),
        'failed', count(*) FILTER (WHERE state = 'failed'),
        'reclaimed', count(*) FILTER (WHERE claims > 1),
        'wait_p95_s', coalesce(percentile_cont(0.95) WITHIN GROUP (ORDER BY extract(epoch FROM claimed_at - enqueued_at)) FILTER (WHERE state = 'done'), 0),
        'wait_max_s', coalesce(max(extract(epoch FROM claimed_at - enqueued_at)) FILTER (WHERE state = 'done'), 0),
        'service_mean_s', coalesce(avg(extract(epoch FROM finished_at - claimed_at)) FILTER (WHERE state = 'done'), 0)
      ) FROM jobs
    ),
    'http', '$http'::json,
    'memory_mib', json_build_object(
      'api_idle', $api_idle, 'api', $(status_mib "$api" VmHWM),
      'judge_each', $judge_each, 'overhead_each', $overhead_each,
      'sandboxes', $sandboxes_peak,
      'postgres_idle', $postgres_idle, 'postgres', $(mib "$postgres/memory.peak")
    ),
    'cpu_s', json_build_object(
      'api', $(awk -v a="$(cpu_of "$api")" -v b="$api_cpu_idle" 'BEGIN { print a - b }'),
      'judges', $judges_cpu,
      'sandboxes', $(awk -v a="$(usage "$sandboxes")" -v b="$sandboxes_cpu_idle" 'BEGIN { print a - b }'),
      'postgres', $(awk -v a="$(usage "$postgres")" -v b="$postgres_cpu_idle" 'BEGIN { print a - b }')
    ),
    'disk_mib', json_build_object(
      'database_growth', (pg_database_size('log791') - $database_idle) / 1048576.0
    ),
    'duration_s', $(awk -v a="$end" -v b="$begin" 'BEGIN { printf "%.1f", a - b }')
  )" >"$output"
