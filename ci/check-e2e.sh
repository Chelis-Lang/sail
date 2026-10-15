#!/usr/bin/env bash
# The required aggregate succeeds only after all complete phases have run.
set -euo pipefail
if [ "$#" -ne 4 ]; then
  echo 'e2e: expected legacy, managed, repair and output phase results' >&2
  exit 1
fi
for result in "$@"; do
  if [ "$result" != success ]; then
    echo "e2e: required phase result was '$result'" >&2
    exit 1
  fi
done
