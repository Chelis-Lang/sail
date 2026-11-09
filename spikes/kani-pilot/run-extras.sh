#!/usr/bin/env bash
# Machine-readable output, batch cost, solver variants, concrete playback and
# the function-contract experiments. Writes everything under WORK/extras.
#
#   spikes/kani-pilot/run-extras.sh WORK
set -uo pipefail
export RUSTC_WRAPPER=
# shellcheck disable=SC1091
. "$HOME/.cargo/env"
work=${1:?work dir}
x="$work/extras"
rm -rf "$x"; mkdir -p "$x"
timed() { # NAME DIR CMD...
  local name=$1 dir=$2; shift 2
  (cd "$dir" && /usr/bin/time -o "$x/$name.time" -f 'wall_s=%e user_s=%U sys_s=%S max_rss_kb=%M' \
    "$@" >"$x/$name.log" 2>&1; echo "exit=$?" >>"$x/$name.time")
  echo "$name: $(tr '\n' ' ' <"$x/$name.time") $(grep -E '^VERIFICATION|^Complete|^error' "$x/$name.log" | sort | uniq -c | tr '\n' ' ')"
}
p="$work/pristine/pilot"

# 1. Machine-readable output.
timed list-json "$p" cargo kani list --format json
cp "$p"/kani-list.json "$x/" 2>/dev/null
timed export-json "$p" cargo kani -Z unstable-options --export-json "$x/export.json"
timed terse "$p" cargo kani --output-format terse
timed old-format "$p" cargo kani --output-format old --harness harnesses::f1_push_never_panics --exact

# 2. Batch cost: every harness in one invocation, sequential then parallel,
#    then a cold build (target removed).
timed batch-seq "$p" cargo kani
timed batch-j8 "$p" cargo kani -j 8 --output-format terse
rm -rf "$p/target"
timed batch-cold "$p" cargo kani

# 3. Solver variants on the most expensive harness.
for s in cadical kissat minisat z3 cvc5 bitwuzla; do
  timed "solver-$s" "$p" cargo kani --harness harnesses::p3_insert_remove_inverse --exact --solver "$s"
done

# 4. Concrete playback for a failing harness.
timed playback "$p" cargo kani -Z concrete-playback --concrete-playback print \
  --harness harnesses::f1_push_never_panics --exact

# 5. Function contracts. `stub_verified` also needs `-Z stubbing`.
c="--features contracts -Z function-contracts -Z stubbing"
# shellcheck disable=SC2086
timed contracts-r1 "$p" cargo kani $c --harness contracts::
# shellcheck disable=SC2086
timed contracts-r2 "$p" cargo kani $c --harness contracts::
timed contracts-no-stubbing-flag "$p" cargo kani --features contracts -Z function-contracts --harness contracts::
timed contracts-no-flags "$p" cargo kani --features contracts --harness contracts::
# `cargo kani list` takes no --features, so feature-gated contracts are invisible to it.
timed contracts-list "$p" cargo kani list -Z function-contracts --format json
cp "$p"/kani-list.json "$x/contracts-list.json" 2>/dev/null
# shellcheck disable=SC2086
timed contract-stub-foreign "$p" cargo kani $c --features contract-stub-foreign \
  --harness contracts::c1_caller_uses_contract_as_stub --exact
# shellcheck disable=SC2086
timed contract-in-crate-r1 "$work/contract/pilot" cargo kani $c --features contract-in-crate \
  --harness contracts::c3_check_in_crate_push_unchecked --exact
# shellcheck disable=SC2086
timed contract-in-crate-r2 "$work/contract/pilot" cargo kani $c --features contract-in-crate \
  --harness contracts::c3_check_in_crate_push_unchecked --exact
# The in-crate contract attributes must not disturb an ordinary build of the dependency.
timed contract-copy-plain-cargo-check "$work/contract/arrayvec-0.7.8" cargo check
echo EXTRASDONE
