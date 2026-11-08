#!/usr/bin/env bash
# Rules of the ADRs that a pattern can check in the code, each with the decision it enforces.
# grep and perl rather than a SAST tool: these are textual rules, and nothing to install.
# Usage: bash tests/policy/code.sh [repository root]
set -uo pipefail
cd "${1:-.}" || exit 1
failed=0

# rule NAME WHY PERL-REGEX FILE...: fails on every file where the multi-line pattern matches
rule() {
  local name=$1 why=$2 pattern=$3
  shift 3
  local hits
  hits=$(for file in "$@"; do [ -f "$file" ] && perl -0777 -ne "print \"\$ARGV\n\" if /$pattern/s" "$file"; done)
  if [ -z "$hits" ]; then
    echo "ok    $name"
  else
    echo "FAIL  $name ($why):"
    echo "      ${hits//$'\n'/$'\n      '}"
    failed=1
  fi
}

mapfile -t rust < <(find apps packages -name '*.rs' -not -path '*/target/*' 2>/dev/null)
mapfile -t page < <(find apps/web/src apps/web/index.html packages/ui/src \
  \( -name '*.svelte' -o -name '*.ts' -o -name '*.js' -o -name '*.html' -o -name '*.css' \) 2>/dev/null)

rule "no shell between the judge and Docker" "argv only, never a shell: ADR-0018" \
  'Command::new\(\s*"(\/(usr\/)?bin\/)?(ba|da|z)?sh"|\.arg\(\s*"-c"\s*\)|\.args\(\s*&?\[[^\]]*"-c"' "${rust[@]}"

# A log line names no one (Law 25): what an operator reads must not identify a student
rule "no identity data in logs" "logs carry no identity: architecture, Logs" \
  '\b(e?print(ln)?|trace|debug|info|warn|error)!\((?:(?!\);).)*?\b(password|passwd|secret|token|account|username|email|matricule|credentials)\b' \
  "${rust[@]}"

rule "no SQL built from strings" "queries are checked by sqlx at compile time" \
  '\bquery(_as|_scalar)?(::<[^>]*>)?\(\s*&?\s*format!' "${rust[@]}"

rule "no raw HTML in the page" "only sanitized statements reach the DOM: ADR-0023" \
  '\{\@html\b' "${page[@]}"

# Under Safe Exam Browser the page loads only from its own origin: no CDN (ADR-0009, ADR-0034)
rule "no other origin in the page" "same origin only: ADR-0009, ADR-0034" \
  '["'"'"'\x60]https?:\/\/' "${page[@]}"

exit "$failed"
