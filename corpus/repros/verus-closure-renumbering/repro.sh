#!/usr/bin/env bash
# Verifies before.rs and after.rs (after.rs adds one function to module m1)
# with --log smt and compares module m2's SMT log, without `;;` comment
# lines. m2 is unchanged, so its solver input should be too. See README.md.
# Usage: repro.sh <verus-install-dir>
set -euo pipefail
install="${1:?usage: repro.sh <verus-install-dir>}"
here="$(cd "$(dirname "$0")" && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
for v in before after; do
  mkdir -p "$work/$v"
  # The same file name, so both runs compile the same crate name.
  cp "$here/$v.rs" "$work/$v/repro.rs"
  (cd "$work/$v" && "$install/verus" repro.rs --log smt --log-dir logs >/dev/null)
  grep -v '^ *;;' "$work/$v/logs/m2.smt2" >"$work/$v.m2.smt2"
done
if cmp -s "$work/before.m2.smt2" "$work/after.m2.smt2"; then
  echo "m2 identical"
else
  echo "m2 differs:"
  diff "$work/before.m2.smt2" "$work/after.m2.smt2" | grep anonymous_closure | head -4 || true
  exit 1
fi
