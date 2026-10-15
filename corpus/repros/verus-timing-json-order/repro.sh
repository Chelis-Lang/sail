#!/usr/bin/env bash
# Runs Verus on test.rs (next to this script) N times with --output-json --time
# and counts the distinct orders of the timing report's module and function
# lists (names only; the times themselves are left out). See README.md.
# Usage: repro.sh <verus-install-dir> [runs]
set -euo pipefail
install="${1:?usage: repro.sh <verus-install-dir> [runs]}"
runs="${2:-10}"
here="$(cd "$(dirname "$0")" && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
cp "$here/test.rs" "$work/test.rs"
cd "$work"
for i in $(seq 1 "$runs"); do
  "$install/verus" --crate-type=lib --output-json --time test.rs >"out-$i.json" 2>/dev/null || true
  python3 -c '
import json, sys
t = json.load(open(sys.argv[1]))["times-ms"]
run = t["smt"]["smt-run-module-times"]
names = [[m["module"] for m in t["total-verify-module-times"]],
         [m["module"] for m in run],
         [[f["function"] for f in m["function-breakdown"]] for m in run]]
print(json.dumps(names))' "out-$i.json" | shasum -a 256 | cut -c1-16
done | sort | uniq -c
