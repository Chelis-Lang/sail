#!/usr/bin/env bash
# SPIKE part 1: the runtime crate's tests, including every solver-backed
# (ignored) test, against the pinned Z3 and the pinned cvc5, three rounds.
# Run from ~/sail after host-setup.sh. Output: ~/spike-out/runtime-tests.log
set -euo pipefail
# shellcheck disable=SC1091
. "$HOME/.cargo/env"
verus_home="$HOME/.cache/sail/verus/0.2026.10.18.7d82994/verus-x86-linux"
export SAIL_TEST_Z3="$verus_home/z3"
export SAIL_TEST_CVC5="$HOME/spike-tools/cvc5-1.4.2/bin/cvc5"
{
  date -u
  sha256sum "$SAIL_TEST_Z3" "$SAIL_TEST_CVC5"
  "$SAIL_TEST_CVC5" --version | head -2
  "$SAIL_TEST_Z3" --version
  for round in 1 2 3; do
    echo "=== round $round"
    cargo test -p sail-runtime -- --include-ignored --nocapture --test-threads=1 2>&1 |
      grep -E '^test |test result|cvc5 case|boundary|panicked|assert|FAILED|error' || true
  done
  date -u
} >"$HOME/spike-out/runtime-tests.log" 2>&1
echo RUNTIME-TESTS-DONE >>"$HOME/spike-out/runtime-tests.log"
