#!/usr/bin/env bash
# Once the platform holds real data, a migration that was released is never edited: a database
# migrated by it would not match the edited file (ADR-0024: expand, then contract, in new files).
# Off until then, so the migrations can still be reworked (MIGRATIONS_FROZEN, a repository variable
# to set to true at the first production use).
# Usage: MIGRATIONS_FROZEN=true bash tests/policy/migrations.sh
set -uo pipefail

if [ "${MIGRATIONS_FROZEN:-}" != true ]; then
  echo "ok    migrations (not frozen yet: MIGRATIONS_FROZEN is not true)"
  exit 0
fi

release=$(git describe --tags --abbrev=0 --match 'v*' 2>/dev/null) || {
  echo "ok    migrations (no release yet)"
  exit 0
}
changed=$(git diff --name-status "$release" -- db/migrations | grep -v '^A')
if [ -n "$changed" ]; then
  echo "FAIL  migrations released in $release changed; add a new migration instead:"
  echo "      ${changed//$'\n'/$'\n      '}"
  exit 1
fi
echo "ok    migrations released in $release are unchanged"
