#!/usr/bin/env bash
# Runs every reference solution in solutions/<exercise>/ on every case of
# examples/content/exercises/<exercise>/assessment/ and compares its output byte for byte.
# Fails on an exercise without a solution, a solution without an exercise, or an unknown language.
set -uo pipefail
cd "$(dirname "$0")" || exit

exercises=../../examples/content/exercises
status=0
fail() { echo "FAIL $*"; status=1; }

for dir in solutions/*/; do
  [ -d "$exercises/$(basename "$dir")" ] || fail "$dir: no such exercise"
done

for exercise in "$exercises"/*/; do
  name=$(basename "$exercise")
  solutions=(solutions/"$name"/*)
  [ -e "${solutions[0]}" ] || { fail "$name: no reference solution"; continue; }

  for solution in "${solutions[@]}"; do
    case $solution in
      *.py) run=(python3 "$solution") ;;
      *.java) run=(java "$solution") ;;
      *.js) run=(node "$solution") ;;
      *) fail "$solution: unknown language"; continue ;;
    esac

    for input in "$exercise"assessment/*/*.in; do
      if "${run[@]}" < "$input" | cmp -s - "${input%.in}.out"; then
        echo "ok   $solution $(basename "$input" .in)"
      else
        fail "$solution $(basename "$input" .in)"
      fi
    done
  done
done

exit "$status"
