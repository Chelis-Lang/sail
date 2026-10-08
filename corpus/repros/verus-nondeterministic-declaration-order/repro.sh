#!/usr/bin/env bash
# Runs the pinned Verus on examples/syntax.rs (from the pinned Verus source)
# N times, recording the exact bytes sent to Z3 through a logging wrapper,
# and reports how many distinct byte streams were produced. See README.md.
# Usage: repro.sh <verus-install-dir> <verus-source-dir> [runs]
set -euo pipefail
install="${1:?usage: repro.sh <verus-install-dir> <verus-source-dir> [runs]}"
source_dir="${2:?usage: repro.sh <verus-install-dir> <verus-source-dir> [runs]}"
runs="${3:-6}"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
cat > "$work/z3" <<WRAP
#!/bin/sh
f="$work/streams/\$\$.smt2"
tee "\$f" | "$install/z3" "\$@"
WRAP
chmod +x "$work/z3"
for i in $(seq 1 "$runs"); do
  mkdir -p "$work/streams"
  VERUS_Z3_PATH="$work/z3" "$install/verus" --crate-type=lib \
    "$source_dir/examples/syntax.rs" >/dev/null 2>&1 || true
  # One digest per run: the sorted digests of every solver process's input.
  for f in "$work/streams"/*.smt2; do shasum -a 256 "$f" | cut -c1-16; done | sort | shasum | cut -c1-16
  rm -rf "$work/streams"
done | sort | uniq -c
