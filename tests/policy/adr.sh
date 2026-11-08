#!/usr/bin/env bash
# An accepted ADR is never edited, only superseded by a new one (docs/adr/index.typ): between
# BASE and HEAD, an ADR accepted at BASE may change its status line and nothing else.
# A deliberate edit says why in a commit of the range, with a trailer "ADR-edit: <reason>".
# Usage: bash tests/policy/adr.sh BASE    (no BASE, or an all-zero one: nothing to compare)
set -uo pipefail
base=${1:-}
case "$base" in '' | 0000000*)
  echo "ok    accepted ADRs (no base to compare with)"
  exit 0
  ;;
esac

reason=$(git log --format=%B "$base..HEAD" | sed -n 's/^ADR-edit: *\(..*\)$/\1/p' | head -1)
failed=0

while read -r file; do
  git show "$base:$file" 2>/dev/null | grep -qi '^\*Status:\* *accepted' || continue
  if [ ! -e "$file" ]; then
    changed="deleted"
  else
    changed=$(diff <(git show "$base:$file" | grep -v '^\*Status:\*') <(grep -v '^\*Status:\*' "$file") >/dev/null || echo "edited")
  fi
  [ -n "$changed" ] || continue
  if [ -n "$reason" ]; then
    echo "ok    $file $changed after acceptance (ADR-edit: $reason)"
  else
    echo "FAIL  $file $changed after acceptance: supersede it with a new ADR, or give a commit an 'ADR-edit: <reason>' trailer"
    failed=1
  fi
done < <(git diff --name-only "$base" -- 'docs/adr/[0-9]*.typ')

[ "$failed" -ne 0 ] || echo "ok    accepted ADRs"
exit "$failed"
