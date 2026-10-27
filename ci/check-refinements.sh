#!/usr/bin/env bash
# Every fixed matrix must pass: each partition checks the whole positive
# root, and the four disjoint control partitions cover its complete inventory.
set -euo pipefail
if [ "$#" -ne 3 ]; then
  echo 'refinements: expected query, inputs and authority results' >&2
  exit 1
fi
for result in "$@"; do
  if [ "$result" != success ]; then
    echo "refinements: required root result was '$result'" >&2
    exit 1
  fi
done
