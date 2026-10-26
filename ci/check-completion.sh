#!/usr/bin/env bash
# A required job must have run successfully, not merely been skipped.
set -euo pipefail
if [ "$#" -ne 8 ]; then
  echo 'completion: expected authorship, image, gate, solver, e2e, conformance, model-proof and production-refinement results' >&2
  exit 1
fi
for result in "$@"; do
  if [ "$result" != success ]; then
    echo "completion: required job result was '$result'" >&2
    exit 1
  fi
done
