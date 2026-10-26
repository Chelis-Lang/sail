#!/usr/bin/env bash
# Every fixed root includes its positive proof and complete mutation inventory.
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
