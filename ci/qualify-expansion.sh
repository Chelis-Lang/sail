#!/usr/bin/env bash
# Local native installed controls; deliberately not a required CI gate.
set -euo pipefail
out="${1:?usage: ci/qualify-expansion.sh EVIDENCE_DIR}"
mkdir -p "$out"
out="$(cd "$out" && pwd)"
work="$(mktemp -d /tmp/sail-expansion.XXXXXX)"
capture() {
  # Keep raw streams/sources outside caches, including unsuccessful attempts.
  rsync -a --exclude cache "$work/" "$out/" || true
  for lane in repair preparation budget; do
    if [ -f "$work/$lane/cache/daemon.log" ]; then
      cp "$work/$lane/cache/daemon.log" "$out/$lane-daemon.log"
    fi
    for worker in "$work/$lane/cache/results/projects/jobs/preparation/stderr" \
                  "$work/$lane/cache/projects/jobs/preparation/stderr"; do
      if [ -f "$worker" ]; then cp "$worker" "$out/$lane-worker.stderr"; fi
    done
  done
}
trap capture EXIT
cargo build --release --locked -p sail-solve -p sail-shim
sail="$(pwd)/target/release/sail"
python3 ci/test-standalone-repair.py --sail "$sail" --stock "$(command -v verus)" \
  --runtime "${SAIL_ENGINE_BUILD:?requires validated runtime}" --out "$work/repair"
python3 ci/test-preparation.py --sail "$sail" --runtime "$SAIL_ENGINE_BUILD" --out "$work/preparation"
python3 ci/test-project-budget.py --sail "$sail" --runtime "$SAIL_ENGINE_BUILD" --out "$work/budget"
