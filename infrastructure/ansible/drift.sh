#!/usr/bin/env bash
# Are the VMs still what site.yml describes (ADR-0006)? Runs it in check mode, which changes
# nothing, and fails if any task would change a host: someone changed it by hand, or a package
# moved under it. From the operator's machine on the VPN; tests/deploy/run.sh runs it too.
# Usage: bash infrastructure/ansible/drift.sh [inventory] [ansible-playbook options...]
set -euo pipefail
cd "$(dirname "$0")"

inventory=${1:-inventory/ets}
log=$(mktemp)
trap 'rm -f "$log"' EXIT

ansible-playbook -i "$inventory" site.yml --check --diff "${@:2}" | tee "$log"
# The operator's machine publishes in check mode too, which is never drift
drifted=$(awk '/^PLAY RECAP/ {recap = 1; next} recap && NF && $1 != "localhost"' "$log" | grep -v ' changed=0 ' || true)
if [ -n "$drifted" ]; then
  echo "FAIL  these hosts differ from site.yml (diffs above):" >&2
  echo "$drifted" >&2
  exit 1
fi
echo "ok    no drift from site.yml"
