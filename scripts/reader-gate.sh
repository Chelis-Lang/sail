#!/usr/bin/env bash
# M7 reader gate (docs/validation/m7.md, gate 5): four clients read the Status
# and complete Result of a Current HumanEval repair job while a second repair
# job verifies. Linux, local-only; run inside the CI image on a native host.
#
#   scripts/reader-gate.sh BIN_DIR PROBE WORKSPACE OUT
#
# BIN_DIR holds `sail` and `sail-z3`; PROBE is the `job_read_bench` example;
# WORKSPACE is the source-only HumanEval workspace; OUT must not exist.
# The engine runtime and stock Verus default to the CI image's paths.
set -euo pipefail
bin="$(cd "${1:?BIN_DIR}" && pwd)"; probe="$(realpath "${2:?PROBE}")"
source="$(cd "${3:?WORKSPACE}" && pwd)"; out="${4:?OUT}"
engine="${SAIL_ENGINE_BUILD:-/opt/sail-engine}"
stock="${SAIL_STOCK_BIN:-/opt/verus/verus-x86-linux}"
package=human_eval_verus
function=human_eval_verus::human_eval_013::compute_gcd
selection=(--package "$package" --lib)

mkdir "$out"; out="$(cd "$out" && pwd)"
export SAIL_CACHE_DIR="$out/cache" SAIL_DAEMON_SOCKET="$out/cache.sock" SAIL_MANAGED_FORCE=1
# Sail runs its own Verus, the fork runtime (ADR 0043).
export SAIL_VERUS_BUILD="$engine" VERUS_Z3_PATH="$stock/z3" PATH="$bin:$PATH"
field() { python3 -c 'import json,sys; v=json.load(sys.stdin)
for k in sys.argv[1:]: v=v[k]
print(v)' "$@"; }

toolchain="$(sail engine install --from "$engine" --output json | field rust_toolchain)"
export RUSTUP_TOOLCHAIN="$toolchain"
for name in baseline active; do
  mkdir "$out/$name"
  tar -C "$source" --exclude=./target --exclude=./.git -cf - . | tar -C "$out/$name" -xf -
done
sail serve --socket "$SAIL_DAEMON_SOCKET" --cache "$SAIL_CACHE_DIR" --idle-timeout 0 \
  >"$out/daemon.log" 2>&1 &
daemon=$!
trap 'kill -INT "$daemon" 2>/dev/null || true' EXIT
for _ in $(seq 100); do [ -S "$SAIL_DAEMON_SOCKET" ] && break; sleep 0.1; done

submit() { (cd "$out/$1" && sail repair-check --engine interactive --function "$function" \
  --background --output json "${selection[@]}" | field id); }
finish() { # wait for job $1 to end; print its result document
  while :; do
    state="$(sail jobs status "$1" --output json | field value state)"
    case "$state" in queued|running) sleep 1 ;; *) break ;; esac
  done
  sail jobs result "$1" --output json
}

# Setup (unmeasured), in each copy: an ordinary full proof, then the repair
# context. A first repair run may only establish build-script declarations;
# a second must be accepted and Current.
warm() {
  (cd "$out/$1" && sail verify --output json "${selection[@]}" >"$out/$1-verify.json")
  for attempt in 1 2; do
    candidate="$(submit "$1")"
    finish "$candidate" >"$out/$1-$attempt.json"
    if [ "$(field value repair accepted <"$out/$1-$attempt.json")" = True ] &&
       [ "$(field value result currentness <"$out/$1-$attempt.json")" = current ]; then
      echo "$candidate"; return
    fi
  done
  echo "reader-gate: no accepted Current repair job in $1" >&2; exit 1
}
job="$(warm baseline)"
warm active >/dev/null

# Measured: a further repair in the second copy is the active verification.
# Start reading once its verifier for the selected crate is live, and require
# the same verifier to be live afterwards.
active="$(submit active)"
verifier() { pgrep -f -- "crate-name.$package( |$)" | head -1; }
for _ in $(seq 6000); do pid="$(verifier || true)"; [ -n "$pid" ] && break; sleep 0.1; done
[ -n "${pid:-}" ] || { echo "reader-gate: the active verifier never started" >&2; exit 1; }
rc=0
"$probe" --socket "$SAIL_DAEMON_SOCKET" --job "$job" --active-job "$active" \
  --out "$out/samples.json" || rc=$?
if ! kill -0 "$pid" 2>/dev/null; then
  echo "reader-gate: the active verifier finished during the census" >&2; rc=1
fi
sail jobs cancel "$active" >/dev/null 2>&1 || true
echo "reader-gate: $([ "$rc" = 0 ] && echo passed || echo FAILED)"
exit "$rc"
