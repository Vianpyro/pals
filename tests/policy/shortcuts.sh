#!/usr/bin/env bash
# The ledger of known shortcuts (ADR-0014: a shortcut is marked, or it is a bug), as Markdown for
# the CI summary. Never fails: it reports what is owed, the code review decides when to pay it.
# Usage: bash tests/policy/shortcuts.sh
set -uo pipefail

entries=$(git grep -nIE '(shortcut|ponytail): ' -- ':!tests/policy/shortcuts.sh' ':!*.md' 2>/dev/null)
echo "### Shortcuts ($(grep -c . <<<"$entries" || true))"
echo
[ -n "$entries" ] || { echo "None."; exit 0; }
echo "| Where | What |"
echo "|---|---|"
while IFS=: read -r file line text; do
  text=${text#*: }
  echo "| \`$file:$line\` | ${text//|/\\|} |"
done <<<"$entries"
