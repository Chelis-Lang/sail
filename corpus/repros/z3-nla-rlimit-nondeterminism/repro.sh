#!/usr/bin/env bash
# Replays query.smt2 into the pinned Z3 N times and tallies the
# rlimit-count replies and check-sat answers. See README.md.
# Usage: repro.sh <path-to-pinned-z3> [runs]
set -euo pipefail
z3="${1:?usage: repro.sh <z3> [runs]}"
runs="${2:-20}"
here="$(cd "$(dirname "$0")" && pwd)"
for _ in $(seq 1 "$runs"); do
  out="$("$z3" -smt2 -in < "$here/query.smt2")"
  counts="$(printf '%s\n' "$out" | grep -oE 'rlimit-count +[0-9]+' | awk '{print $2}' | paste -sd' ' -)"
  answers="$(printf '%s\n' "$out" | grep -E '^(sat|unsat|unknown)$' | paste -sd' ' -)"
  echo "rlimit-counts: $counts | answers: $answers"
done | sort | uniq -c
