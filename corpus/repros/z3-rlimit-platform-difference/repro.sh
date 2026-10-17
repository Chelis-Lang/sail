#!/usr/bin/env bash
# Runs Verus on test.rs (next to this script) N times and prints each run's
# verification summary and any rlimit error. See README.md.
# Usage: repro.sh <verus-install-dir> [runs]
set -euo pipefail
install="${1:?usage: repro.sh <verus-install-dir> [runs]}"
runs="${2:-3}"
here="$(cd "$(dirname "$0")" && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
cp "$here/test.rs" "$work/test.rs"
cd "$work"
"$install/z3" --version
for i in $(seq 1 "$runs"); do
  "$install/verus" --crate-type=lib test.rs 2>&1 |
    grep -E 'verification results|rlimit' | tr '\n' ' ' || true
  echo
done | sort | uniq -c
