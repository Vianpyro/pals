#!/usr/bin/env bash
# Every language pack names its image by digest, so a tag moved upstream never changes what a
# student's program runs on between two exams (ADR-0014), and has its conformance suite (ADR-0013).
# Usage: bash tests/policy/packs.sh [repository root]
set -uo pipefail
cd "${1:-.}" || exit 1
failed=0
fail() { echo "FAIL  $1"; failed=1; }

for pack in packs/languages/*/; do
  name=$(basename "$pack")
  image=$(sed -n 's/^image = "\(.*\)"$/\1/p' "$pack/pack.toml" 2>/dev/null)
  if [[ ! $image =~ ^[a-z0-9][a-z0-9._/-]*:[A-Za-z0-9][A-Za-z0-9._-]*@sha256:[0-9a-f]{64}$ ]]; then
    fail "$name: image '$image' is not name:tag@sha256:<digest>"
    continue
  fi
  for verdict in passed wrong-answer crashed timed-out; do
    [ -n "$(ls -A "$pack/conformance/$verdict" 2>/dev/null)" ] || { fail "$name: no conformance/$verdict program"; continue 2; }
  done
  echo "ok    pack $name"
done

exit "$failed"
