#!/usr/bin/env bash
# Runs the pinned Verus on test.rs (next to this script) N times with a given
# --num-threads, and reports how many distinct stderr outputs were produced.
# See README.md.
# Usage: repro.sh <verus-install-dir> [runs] [num-threads]
set -euo pipefail
install="${1:?usage: repro.sh <verus-install-dir> [runs] [num-threads]}"
runs="${2:-12}"
threads="${3:-8}"
here="$(cd "$(dirname "$0")" && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
cp "$here/test.rs" "$work/test.rs"
cd "$work"
for i in $(seq 1 "$runs"); do
  "$install/verus" --crate-type=lib --num-threads "$threads" test.rs \
    >/dev/null 2>"stderr-$i.txt" || true
  shasum -a 256 "stderr-$i.txt" | cut -c1-16
done | sort | uniq -c
