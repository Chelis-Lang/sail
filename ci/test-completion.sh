#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
bash ci/check-completion.sh success success success success success success success success
for result in failure cancelled skipped '' pending Success; do
  for index in 0 1 2 3 4 5 6 7; do
    results=(success success success success success success success success)
    results[$index]="$result"
    if bash ci/check-completion.sh "${results[@]}" 2>/dev/null; then
      echo "completion incorrectly accepted '$result' at $index" >&2
      exit 1
    fi
  done
done
if bash ci/check-completion.sh success 2>/dev/null; then
  echo 'completion incorrectly accepted missing results' >&2
  exit 1
fi
if bash ci/check-completion.sh success success success success success success success 2>/dev/null; then
  echo 'completion incorrectly accepted an omitted production-refinement result' >&2
  exit 1
fi

# Keep the End to end aggregate fail-closed independently of CI completion.
bash ci/check-e2e.sh success success success success success success success success success success success
for result in failure cancelled skipped '' pending Success; do
  for index in 0 1 2 3 4 5 6 7 8 9 10; do
    phases=(success success success success success success success success success success success)
    phases[$index]="$result"
    if bash ci/check-e2e.sh "${phases[@]}" 2>/dev/null; then
      echo "e2e aggregate incorrectly accepted '$result' at $index" >&2
      exit 1
    fi
  done
done
for count in 0 1 9 10 12; do
  phases=()
  for ((index=0; index<count; index++)); do phases+=(success); done
  # Portable to macOS Bash 3.2, where an empty array is unset under nounset.
  if bash ci/check-e2e.sh ${phases[@]+"${phases[@]}"} 2>/dev/null; then
    echo "e2e aggregate incorrectly accepted $count phase results" >&2
    exit 1
  fi
done

# The proof aggregate must reject any unrun root, independently of completion.
bash ci/check-refinements.sh success success success
for result in failure cancelled skipped '' pending Success; do
  for index in 0 1 2; do
    roots=(success success success)
    roots[$index]="$result"
    if bash ci/check-refinements.sh "${roots[@]}" 2>/dev/null; then
      echo "refinement aggregate incorrectly accepted '$result' at $index" >&2
      exit 1
    fi
  done
done
for count in 0 1 2 4; do
  roots=()
  for ((index=0; index<count; index++)); do roots+=(success); done
  if bash ci/check-refinements.sh ${roots[@]+"${roots[@]}"} 2>/dev/null; then
    echo "refinement aggregate incorrectly accepted $count roots" >&2
    exit 1
  fi
done
