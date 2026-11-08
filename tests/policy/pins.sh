#!/usr/bin/env bash
# One version of Rust and one of Node, wherever they are written: Renovate moves the copies
# together (.github/renovate.json5), this catches a copy edited by hand.
# Usage: bash tests/policy/pins.sh [repository root]
set -uo pipefail
cd "${1:-.}" || exit 1
failed=0

# same NAME VALUE...: every value equals the first; each value is "place=version"
same() {
  local name=$1 first=${2#*=}
  shift
  for pin in "$@"; do
    if [ "${pin#*=}" != "$first" ]; then
      echo "FAIL  $name: $(printf '%s ' "$@")"
      failed=1
      return
    fi
  done
  echo "ok    $name $first"
}

# \r*: a Windows working copy may have CRLF line endings, the repository has LF
rust=("rust-toolchain.toml=$(sed -n 's/^channel = "\(.*\)"\r*$/\1/p' rust-toolchain.toml)"
  "Cargo.toml=$(sed -n 's/^rust-version = "\(.*\)"\r*$/\1/p' Cargo.toml)"
  ".devcontainer/devcontainer.json=$(sed -n 's/.*features\/rust:1": { "version": "\([^"]*\)".*/\1/p' .devcontainer/devcontainer.json)")
for dockerfile in apps/*/Dockerfile; do
  version=$(sed -n 's/^FROM rust:\([^-]*\)-.*/\1/p' "$dockerfile")
  [ -z "$version" ] || rust+=("$dockerfile=$version")
done
same rust "${rust[@]}"

same node \
  "apps/web/Dockerfile=$(sed -n 's/^FROM node:\([^-]*\)-.*/\1/p' apps/web/Dockerfile)" \
  ".devcontainer/Dockerfile=$(sed -n 's/^FROM node:\([^-]*\)-.*/\1/p' .devcontainer/Dockerfile)" \
  "package.json=$(sed -n 's/.*"node": ">=\([^ "]*\).*/\1/p' package.json)"

exit "$failed"
