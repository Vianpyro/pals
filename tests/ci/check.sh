#!/usr/bin/env bash
# Checks .github/changes.sh on a small paths file written here, then that every area the
# workflows read is in .github/paths. Run from the repository root: bash tests/ci/check.sh
set -uo pipefail

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
failed=0

cat >"$work/paths" <<'EOF'
# a comment
rust apps/api/*
rust Cargo.lock

web apps/web/*
EOF

# expect DESCRIPTION WANTED [FILE...]: WANTED is the output, areas joined by spaces
expect() {
  local description=$1 want=$2
  shift 2
  local got
  got=$(printf '%s\n' "$@" | bash .github/changes.sh "$work/paths" | tr '\n' ' ')
  if [ "$got" = "$want " ]; then
    echo "ok    $description"
  else
    echo "FAIL  $description: got '$got', want '$want'"
    failed=1
  fi
}

expect "nothing changed" "rust=false web=false"
expect "a file deep under a pattern" "rust=true web=false" apps/api/src/main.rs
expect "an exact file" "rust=true web=false" Cargo.lock
expect "a file of no area" "rust=false web=false" README.md
expect "a prefix is not a directory" "rust=false web=false" apps/api-other/x
expect "two areas" "rust=true web=true" apps/web/a.ts apps/api/b.rs
expect "a workflow change runs everything" "rust=true web=true" .github/workflows/ci.yml
# As the changes job of ci.yml runs it on a push: no file on stdin
got=$(ALL=1 bash .github/changes.sh "$work/paths" </dev/null | tr '\n' ' ')
if [ "$got" = "rust=true web=true " ]; then echo "ok    ALL=1 runs everything"; else echo "FAIL  ALL=1: got '$got'"; failed=1; fi

# Every steps.areas.outputs.<area> the changes job of ci.yml reads is an area of .github/paths
known=$(awk '!/^#/ && NF {print $1}' .github/paths | sort -u)
while read -r area; do
  if grep -qx "$area" <<<"$known"; then
    echo "ok    area $area is in .github/paths"
  else
    echo "FAIL  area $area is read by a workflow but missing from .github/paths"
    failed=1
  fi
done < <(grep -ho 'steps\.areas\.outputs\.[a-z]*' .github/workflows/*.yml | sed 's/.*\.//' | sort -u)

exit "$failed"
