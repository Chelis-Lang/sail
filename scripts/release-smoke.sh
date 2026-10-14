#!/usr/bin/env bash
# Smoke-tests one release archive in a fresh directory (docs/releasing.md):
#
#   scripts/release-smoke.sh ARCHIVE [--no-engine]
#
# Checks ARCHIVE against ARCHIVE.sha256, unpacks it, and with an empty
# SAIL_CACHE_DIR checks that `sail engine status` selects the bundled engine,
# that `sail verify` succeeds on a new `cargo verus new` project, and that one
# `sail repair-check` through the engine is accepted. `--no-engine` (an
# archive built without the engine) instead requires `engine status` to
# report SAIL3120 and skips the repair. Needs the user prerequisites only:
# the pinned stock Verus (`cargo-verus` on PATH) and its Rust toolchain.
set -euo pipefail

archive="$(cd "$(dirname "${1:?usage: scripts/release-smoke.sh ARCHIVE [--no-engine]}")" && pwd)/$(basename "$1")"
engine=1
[ "${2:-}" = --no-engine ] && engine=0
# A short path: the job socket lives under SAIL_CACHE_DIR, and macOS's
# default TMPDIR makes it longer than a Unix socket path allows (SAIL0810).
work="$(mktemp -d /tmp/sail-smoke.XXXXXX)"
echo "release-smoke: working in $work"

(cd "$(dirname "$archive")" && shasum -a 256 -c "$(basename "$archive").sha256")
tar -C "$work" -xzf "$archive"
release="$(cd "$work"/sail-*/ && pwd -P)"
sail="$release/bin/sail"
export SAIL_CACHE_DIR="$work/cache"
"$sail" --version

if [ "$engine" = 1 ]; then
  status="$("$sail" engine status --output json)"
  case "$status" in
    *"\"build\":\"$release/engine/"*) echo "engine: bundled runtime selected" ;;
    *) echo "release-smoke: sail did not select the bundled engine: $status" >&2; exit 1 ;;
  esac
elif "$sail" engine status --output json 2>"$work/engine.err"; then
  echo "release-smoke: an engine was found but none was expected" >&2; exit 1
else
  grep -q SAIL3120 "$work/engine.err"
  echo "engine: absent (SAIL3120), as expected"
fi

cd "$work"
cargo verus new --lib demo >/dev/null
cd demo
cat >src/lib.rs <<'RS'
use vstd::prelude::*;

verus! {

pub fn max(a: u32, b: u32) -> (r: u32)
    ensures r >= a, r >= b, r == a || r == b,
{ if a > b { a } else { b } }

pub proof fn lemma(x: int) ensures x + 0 == x { }

} // verus!
RS
"$sail" verify
echo "verify: ok"

if [ "$engine" = 1 ]; then
  # Exit 0 is acceptance and 1 rejection; the document says why.
  "$sail" repair-check --package demo --lib --function demo::max \
    --engine interactive --output json >"$work/repair.json" || [ $? = 1 ]
  python3 -I - "$work/repair.json" <<'PY'
import json, sys
value = json.load(open(sys.argv[1]))
batch, full = value["batch"], value["batch"]["full"]
assert batch.get("engine_fault") is None and batch.get("selection_error") is None, batch
assert (full.get("engine") or {}).get("name") == "interactive", full.get("engine")
assert value["accepted"] is True, "repair not accepted: currentness " + full["currentness"]
print("repair-check: accepted through the bundled engine")
PY
fi
echo "release-smoke: pass ($work)"
