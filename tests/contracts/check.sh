#!/usr/bin/env bash
# Checks compatible.sh on small bindings written here, so it is tested before any release has some.
# Needs the npm workspace installed (TypeScript). Run from the repository root: bash tests/contracts/check.sh
set -uo pipefail

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
failed=0

# release NAME FILE=CONTENT...: a bindings directory as ts-rs writes it
release() {
  local dir="$work/$1"
  shift
  mkdir -p "$dir"
  for file in "$@"; do printf '%s\n' "${file#*=}" >"$dir/${file%%=*}.ts"; done
}

# expect ok|breaks DESCRIPTION OLD NEW [ACCEPTED]
expect() {
  local want=$1 description=$2 accepted=${5:-/dev/null} got=ok
  CONTRACTS_ACCEPTED=$accepted bash tests/contracts/compatible.sh "$work/$3" "$work/$4" >"$work/out" 2>&1 || got=breaks
  if [ "$got" = "$want" ]; then
    echo "ok    $description"
  else
    echo "FAIL  $description: expected $want, got $got"
    sed 's/^/      /' "$work/out"
    failed=1
  fi
}

run_v1='export type RunStatus = { "state": "waiting" } | { "state": "done", passed: boolean };'
release v1 "RunStatus=$run_v1" 'Submission=export type Submission = { source: string };'

release same "RunStatus=$run_v1" 'Submission=export type Submission = { source: string };'
expect ok "same types" v1 same

release sent-gains-field \
  'RunStatus=export type RunStatus = { "state": "waiting", queued: number } | { "state": "done", passed: boolean };' \
  'Submission=export type Submission = { source: string };'
expect ok "a type the API sends gains a field" v1 sent-gains-field

release sent-loses-field \
  'RunStatus=export type RunStatus = { "state": "waiting" } | { "state": "done" };' \
  'Submission=export type Submission = { source: string };'
expect breaks "a type the API sends loses a field" v1 sent-loses-field

release sent-gains-state \
  'RunStatus=export type RunStatus = { "state": "waiting" } | { "state": "done", passed: boolean } | { "state": "cancelled" };' \
  'Submission=export type Submission = { source: string };'
expect breaks "a type the API sends gains a state the old interface does not know" v1 sent-gains-state
printf 'RunStatus  # the old interface shows unknown states with its fallback text\n' >"$work/accepted"
expect ok "the same, accepted for this release" v1 sent-gains-state "$work/accepted"

release received-gains-required \
  "RunStatus=$run_v1" 'Submission=export type Submission = { source: string, language: string };'
expect breaks "a type the API receives requires a new field" v1 received-gains-required

release received-gains-optional \
  "RunStatus=$run_v1" 'Submission=export type Submission = { source: string, language?: string };'
expect ok "a type the API receives gains an optional field" v1 received-gains-optional

release removed 'Submission=export type Submission = { source: string };'
expect breaks "a type disappears" v1 removed

exit "$failed"
