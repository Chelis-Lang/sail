#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
bash ci/check-completion.sh success success success success success success
for result in failure cancelled skipped ''; do
  for index in 0 1 2 3 4 5; do
    results=(success success success success success success)
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
