#!/usr/bin/env bash
# Checks the headers nginx sends for each kind of response (spec D20, D21): an nginx location that
# sets its own add_header drops the server's, so a page could lose its CSP with nothing visibly broken.
# Usage: bash tests/web/headers.sh [base URL]; from the dev container, the default reaches nginx.
# -k: the dev certificate is self-signed, and this checks headers, not certificates.
set -u

base=${1:-http://localhost}
csp="default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self'; worker-src 'self'; object-src 'none'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'"
failed=0

# expect PATH STATUS CACHE-CONTROL: the status ('any' to skip it), the cache rule ('' for none)
# and the security headers
expect() {
  local path=$1 status=$2 cache=$3 headers
  headers=$(curl -sk -o /dev/null -D - "$base$path" | tr -d '\r')
  local problems=()
  [ "$status" = any ] || [[ $headers == "HTTP/1.1 $status"* ]] || problems+=("status $(head -1 <<<"$headers")")
  grep -qxF "Content-Security-Policy: $csp" <<<"$headers" || problems+=("CSP")
  grep -qxF "X-Content-Type-Options: nosniff" <<<"$headers" || problems+=("nosniff")
  grep -qxF "Referrer-Policy: no-referrer" <<<"$headers" || problems+=("Referrer-Policy")
  if [ -n "$cache" ]; then
    grep -qxF "Cache-Control: $cache" <<<"$headers" || problems+=("Cache-Control")
  elif grep -qi "^Cache-Control:" <<<"$headers"; then
    problems+=("unexpected Cache-Control")
  fi
  if [ ${#problems[@]} -eq 0 ]; then
    echo "ok    $path"
  else
    echo "FAIL  $path: ${problems[*]}"
    failed=1
  fi
}

page=$(curl -sk "$base/")
asset=$(grep -o '/assets/[^"]*\.js' <<<"$page" | head -1)
if [ -z "$asset" ]; then
  echo "FAIL  / links no /assets/ script: build apps/web first (npm run build -w apps/web)"
  exit 1
fi

expect / 200 "no-cache"
# A route of the page, not a file: the fallback serves the page
expect /exercises/sum 200 "no-cache"
expect "$asset" 200 "public, max-age=31536000, immutable"
# A missing hashed file is a real 404, never the page, and is not cached (D21)
expect /assets/missing-0000.js 404 ""
expect /runtimes/missing/0.0.0/missing.wasm 404 ""
# The API's responses carry the headers too, and so does nginx's 502 when the API is not running (CI)
expect /api/missing any ""

exit "$failed"
