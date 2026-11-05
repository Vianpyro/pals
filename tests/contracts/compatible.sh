#!/usr/bin/env bash
# The API of a release must work with the web interface of the release before (ADR-0024, spec D22):
# a tab opened before an update keeps running the old interface against the new API.
# The TypeScript compiler compares packages/contracts/bindings with the previous release's:
# - a type the API sends: the old interface must read the new one (New extends Old), so it may
#   gain fields but not lose them, nor gain a state the old interface does not know;
# - a type the API receives: it must accept what the old interface sends (Old extends New).
# A break made on purpose (a new state the old interface shows with its fallback text, spec D22) is
# listed in tests/contracts/accepted with its reason, and the list is emptied after the release.
#
# Usage, from the repository root: bash tests/contracts/compatible.sh [OLD_BINDINGS NEW_BINDINGS]
# Without arguments: the latest release before HEAD against the working tree. Needs the npm workspace.
set -euo pipefail

# Types the API receives; it sends every other one. Id goes both ways.
received=" Credentials DraftWrite Submission Id "
sent_too=" Id "
accepted=${CONTRACTS_ACCEPTED:-tests/contracts/accepted}

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

if [ $# -eq 2 ]; then
  cp -r "$1" "$work/old"
  cp -r "$2" "$work/new"
else
  release=$(git tag --merged HEAD --no-contains HEAD --sort=-v:refname 'v*' | head -1)
  if [ -z "$release" ] || ! git archive "$release" packages/contracts/bindings 2>/dev/null | tar -x -C "$work" 2>/dev/null; then
    echo "ok    contracts: no earlier release has TypeScript bindings"
    exit 0
  fi
  mv "$work/packages/contracts/bindings" "$work/old"
  cp -r packages/contracts/bindings "$work/new"
fi

is_accepted() { grep -qE "^$1([[:space:]]|$)" "$accepted" 2>/dev/null; }

{
  echo "type Readable<Reader, Written> = [Written] extends [Reader] ? true : false;"
  for file in "$work"/old/*.ts; do
    name=$(basename "$file" .ts)
    is_accepted "$name" && continue
    echo "import type { $name as Old$name } from \"./old/$name\";"
    echo "import type { $name as New$name } from \"./new/$name\";"
    if [[ $received == *" $name "* ]]; then
      echo "export const ${name}_received_from_the_old_interface: Readable<New$name, Old$name> = true;"
    fi
    if [[ $received != *" $name "* || $sent_too == *" $name "* ]]; then
      echo "export const ${name}_sent_to_the_old_interface: Readable<Old$name, New$name> = true;"
    fi
  done
} >"$work/check.ts"

# --pretty prints the failing line, which names the type and the direction
if npx --no -- tsc --noEmit --strict --pretty --module esnext --moduleResolution bundler --target es2022 "$work/check.ts"; then
  echo "ok    contracts: compatible with the previous release's web interface"
else
  echo "FAIL  contracts: the previous release's web interface would break (see above, or tests/contracts/accepted)"
  exit 1
fi
