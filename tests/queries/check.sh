#!/usr/bin/env bash
# Regenerates .sqlx/ from the queries and the migrated schema at DATABASE_URL, then fails
# if it differs from the committed files. Also the way to update .sqlx/ by hand.
# Not `cargo sqlx prepare --check`: when it cannot touch the sources' mtime (the Windows
# bind mount of the dev container), it runs cargo clean, which deletes its own output directory.
# Run from the repository root: bash tests/queries/check.sh
set -euo pipefail

rm -rf .sqlx
mkdir .sqlx

# The macros write .sqlx/ only while they expand: these crates must recompile
cargo clean --package api --package judge
SQLX_OFFLINE_DIR="$PWD/.sqlx" cargo check --locked --workspace

# Against the index, not HEAD: files staged and not yet committed already count
changes=$(git diff --name-status -- .sqlx; git ls-files --others --exclude-standard -- .sqlx | sed 's/^/?\t/')
if [ -n "$changes" ]; then
  echo "$changes"
  echo "FAIL  .sqlx/ did not match the queries: stage the regenerated files above"
  exit 1
fi
echo "ok    .sqlx/"
