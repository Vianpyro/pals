#!/usr/bin/env bash
# Checks the policy checks on small trees and repositories written here: each one passes a good
# case and fails the bad one it exists for. Run from the repository root: bash tests/policy/check.sh
set -uo pipefail

root=$PWD
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
failed=0

# expect ok|fails DESCRIPTION COMMAND...
expect() {
  local want=$1 description=$2 got=ok
  shift 2
  "$@" >"$work/out" 2>&1 || got=fails
  if [ "$got" = "$want" ]; then
    echo "ok    $description"
  else
    echo "FAIL  $description: expected $want, got $got"
    sed 's/^/      /' "$work/out"
    failed=1
  fi
}

# file PATH CONTENT: PATH under the current tree, created with its directories
file() { mkdir -p "$(dirname "$tree/$1")" && printf '%s\n' "$2" >"$tree/$1"; }

# --- pins.sh
tree=$work/pins
file rust-toolchain.toml 'channel = "1.99.1"'
file Cargo.toml 'rust-version = "1.99.1"'
file .devcontainer/devcontainer.json '    "ghcr.io/devcontainers/features/rust:1": { "version": "1.99.1" }'
file apps/api/Dockerfile 'FROM rust:1.99.1-slim-bookworm AS build'
file apps/web/Dockerfile 'FROM node:26.13.0-alpine AS build'
file .devcontainer/Dockerfile 'FROM node:26.13.0-trixie-slim AS node'
file package.json '    "node": ">=26.13.0 <27"'
expect ok "pins: one Rust, one Node" bash "$root/tests/policy/pins.sh" "$tree"
file apps/api/Dockerfile 'FROM rust:1.98.0-slim-bookworm AS build'
expect fails "pins: a Dockerfile left on another Rust" bash "$root/tests/policy/pins.sh" "$tree"
file apps/api/Dockerfile 'FROM rust:1.99.1-slim-bookworm AS build'
file package.json '    "node": ">=26.12.0 <27"'
expect fails "pins: engines left on another Node" bash "$root/tests/policy/pins.sh" "$tree"

# --- packs.sh
tree=$work/packs
digest=sha256:$(printf '0%.0s' {1..64})
file packs/languages/python/pack.toml "image = \"python:3.15-alpine@$digest\""
for verdict in passed wrong-answer crashed timed-out; do file "packs/languages/python/conformance/$verdict/main.py" 'pass'; done
expect ok "packs: pinned by digest, with its conformance suite" bash "$root/tests/policy/packs.sh" "$tree"
file packs/languages/python/pack.toml 'image = "python:3.15-alpine"'
expect fails "packs: an image by tag only" bash "$root/tests/policy/packs.sh" "$tree"
file packs/languages/python/pack.toml "image = \"python:3.15-alpine@$digest\""
rm -r "$tree/packs/languages/python/conformance/timed-out"
expect fails "packs: a conformance verdict missing" bash "$root/tests/policy/packs.sh" "$tree"

# --- code.sh: one clean tree, then each rule broken on its own
clean() {
  rm -rf "$tree"
  file apps/judge/src/sandbox.rs 'let docker = Command::new("docker").args(["run", "--rm"]);
eprintln!("judge: job {}: {error}", job.id);
let row = sqlx::query("SELECT 1").fetch_one(&pool);'
  file apps/web/src/App.svelte '<p>{text}</p>
<script>const api = "/api/health";</script>'
}
tree=$work/code
clean
expect ok "code: a clean tree" bash "$root/tests/policy/code.sh" "$tree"
file apps/judge/src/run.rs 'let child = Command::new("sh").arg("-c").arg(script);'
expect fails "code: a shell between the judge and Docker" bash "$root/tests/policy/code.sh" "$tree"
clean
file apps/judge/src/run.rs 'let child = tokio::process::Command::new("docker").args(["sh", "-c", x]);'
expect fails "code: -c in an argv" bash "$root/tests/policy/code.sh" "$tree"
clean
file apps/api/src/session.rs 'eprintln!(
    "api: sign-in of {}: {error}",
    username
);'
expect fails "code: a log line naming an account, over several lines" bash "$root/tests/policy/code.sh" "$tree"
clean
file apps/api/src/runs.rs 'let rows = sqlx::query(&format!("SELECT * FROM {table}"));'
expect fails "code: SQL built with format!" bash "$root/tests/policy/code.sh" "$tree"
clean
file apps/web/src/Statement.svelte '{@html statement}'
expect fails "code: raw HTML in the page" bash "$root/tests/policy/code.sh" "$tree"
clean
file apps/web/src/lib/editor.ts 'import("https://cdn.example.org/editor.js");'
expect fails "code: a script from another origin" bash "$root/tests/policy/code.sh" "$tree"

# --- adr.sh and migrations.sh, on a throwaway repository
repo() {
  rm -rf "$work/repo"
  git init -q "$work/repo"
  cd "$work/repo" || exit 1
  git config user.name test
  git config user.email test@example.com
  git config core.autocrlf false
  mkdir -p docs/adr db/migrations
  printf '== ADR-0001\n\n*Status:* accepted.\n\nBody.\n' >docs/adr/0001-queue.typ
  printf '== ADR-0002\n\n*Status:* proposed.\n\nBody.\n' >docs/adr/0002-other.typ
  printf 'CREATE TABLE jobs ();\n' >db/migrations/0001_jobs.sql
  git add --all
  git commit -qm 'docs: init'
  git tag v0.1.0
  base=$(git rev-parse HEAD)
}
commit() { git commit -qam "$1"; }

repo
sed -i 's/Body./Changed./' docs/adr/0002-other.typ && commit 'docs: refine proposal'
expect ok "adr: a proposed ADR is edited" bash "$root/tests/policy/adr.sh" "$base"
sed -i 's/accepted./superseded by ADR-0003./' docs/adr/0001-queue.typ && commit 'docs: supersede adr-0001'
expect ok "adr: an accepted ADR is superseded" bash "$root/tests/policy/adr.sh" "$base"
sed -i 's/Body./Changed./' docs/adr/0001-queue.typ && commit 'docs: rewrite adr-0001'
expect fails "adr: an accepted ADR is edited" bash "$root/tests/policy/adr.sh" "$base"
git commit -q --allow-empty -m $'docs: translate adr-0001\n\nADR-edit: translation, same decision'
expect ok "adr: the same, with an ADR-edit reason" bash "$root/tests/policy/adr.sh" "$base"
expect ok "adr: no base" bash "$root/tests/policy/adr.sh" ""

repo
printf 'CREATE TABLE jobs (id int);\n' >db/migrations/0001_jobs.sql && commit 'feat: edit migration'
expect ok "migrations: edited before production" env MIGRATIONS_FROZEN= bash "$root/tests/policy/migrations.sh"
expect fails "migrations: edited once frozen" env MIGRATIONS_FROZEN=true bash "$root/tests/policy/migrations.sh"
git checkout -q "$base" -- db/migrations
printf 'ALTER TABLE jobs ADD id int;\n' >db/migrations/0002_id.sql && git add --all && commit 'feat: add migration'
expect ok "migrations: a new one once frozen" env MIGRATIONS_FROZEN=true bash "$root/tests/policy/migrations.sh"
cd "$root" || exit 1

exit "$failed"
