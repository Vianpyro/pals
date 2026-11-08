#!/usr/bin/env bash
# Every policy check of tests/policy, after their own self-test. The policy job of ci.yml runs it;
# locally, from the repository root: BASE=origin/main bash tests/policy/run.sh
set -uo pipefail
cd "$(dirname "$0")/../.." || exit 1
failed=0

for check in "bash tests/policy/check.sh" "bash tests/policy/pins.sh" "bash tests/policy/packs.sh" \
  "bash tests/policy/code.sh" "bash tests/policy/adr.sh ${BASE:-}" "bash tests/policy/migrations.sh"; do
  echo "== $check"
  $check || failed=1
done

exit "$failed"
