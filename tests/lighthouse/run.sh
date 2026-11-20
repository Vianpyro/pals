#!/usr/bin/env bash
# Measures the demo page with Lighthouse on one screen profile and holds it to the budgets of
# tests/lighthouse/budgets.mjs: builds the demo, installs the tool from its own lockfile, checks its
# verdict logic, then runs the student's flow (flow.mjs). Run by the lighthouse job of ci.yml, and by
# hand from the dev container, after npm ci at the root.
# Usage: bash tests/lighthouse/run.sh [laptop|lab]    LH_RUNS=1 for a quicker look; the reports go
#        to $LH_OUT (a temporary folder by default), one HTML file per run.
set -euo pipefail
cd "$(dirname "$0")/../.."

profile=${1:-laptop}

echo "==> demo build"
npm run build:demo -w apps/web --silent >/dev/null

echo "==> Lighthouse, from tests/lighthouse/package-lock.json"
npm ci --prefix tests/lighthouse --no-audit --no-fund --silent
# As apps/web/Dockerfile does for the page's packages: each one as the registry signed it
npm audit signatures --prefix tests/lighthouse | grep -v '^$'

echo "==> verdict logic"
node --test tests/lighthouse/evaluate.test.mjs

# The Chromium the e2e journeys run in, pinned by @playwright/test: a runner image update cannot
# move the scores with no commit here
if [ -z "${CHROME_PATH:-}" ]; then
  CHROME_PATH=$(node -p 'require("@playwright/test").chromium.executablePath()')
fi
[ -x "$CHROME_PATH" ] || {
  echo "FAIL  no browser at $CHROME_PATH: run npx playwright install chromium" >&2
  exit 1
}

echo "==> flow on $profile ($("$CHROME_PATH" --version))"
LH_PROFILE=$profile CHROME_PATH=$CHROME_PATH node tests/lighthouse/flow.mjs
