#!/usr/bin/env bash
# Everything that needs the dev container's services: PostgreSQL, LDAP, gVisor and nginx, checked
# by .devcontainer/check.sh when the container starts. Run by the integration job of ci.yml, and
# by hand from the dev container: bash tests/ci/integration.sh
# Needs the release tags (fetch-depth: 0): the contracts are compared with the previous release's.
set -euo pipefail
cd "$(dirname "$0")/../.."

step() {
  echo
  echo "==> $1"
  shift
  "$@" || { echo "FAIL  integration step: $*" >&2; exit 1; }
}

wait_for_api() {
  timeout 300 sh -c 'until curl -sf http://localhost/api/health >/dev/null; do sleep 2; done' && return
  cat /tmp/demo.log
  return 1
}

# The queries against the migrated schema: .sqlx/ must match it
step ".sqlx matches the queries" bash tests/queries/check.sh
# The tests ignored by the rust job need PostgreSQL, LDAP or runsc: the judge's conformance suite
step "tests that need the services" cargo test --locked -- --ignored
step "contract checker" bash tests/contracts/check.sh
step "contracts compatible with the previous release" bash tests/contracts/compatible.sh
step "page build" npm run build -w apps/web
step "nginx headers on the built page" bash tests/web/headers.sh

# The student's journeys need the API and the judge: the demo runs in the background
bash .devcontainer/demo.sh >/tmp/demo.log 2>&1 &
step "API answers" wait_for_api
step "end-to-end journeys" env CI=1 npm run e2e
step "demo journey" env CI=1 npm run e2e:demo
