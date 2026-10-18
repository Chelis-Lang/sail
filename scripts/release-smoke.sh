#!/usr/bin/env bash
# Smoke-tests one release archive in a fresh directory (docs/releasing.md):
#
#   scripts/release-smoke.sh ARCHIVE
#
# Checks ARCHIVE against ARCHIVE.sha256, unpacks it, and with an empty
# SAIL_CACHE_DIR checks that `sail engine status` selects the bundled Verus
# runtime, that `sail verify` succeeds on a new project written as the
# README's First verification says, and that one `sail repair-check` through
# the engine is accepted. Needs the user prerequisites only: rustup and the
# Rust toolchain Verus pins. No Verus installation is used (ADR 0043): PATH
# is checked to hold no `cargo-verus`, and SAIL_VERUS_BUILD and
# SAIL_CARGO_VERUS are unset.
set -euo pipefail

# Factory/smoke tooling only; installed Sail does not invoke Python.
release_python="${SAIL_RELEASE_PYTHON:-python3}"
"$release_python" -c 'import sys; sys.exit(0 if sys.version_info >= (3, 12) else "release tooling requires Python 3.12 or newer; set SAIL_RELEASE_PYTHON")'

release_helper="$(cd "$(dirname "$0")" && pwd -P)/release-shipped.py"
archive="$(cd "$(dirname "${1:?usage: scripts/release-smoke.sh ARCHIVE}")" && pwd)/$(basename "$1")"
unset SAIL_VERUS_BUILD SAIL_ENGINE_BUILD SAIL_CARGO_VERUS SAIL_VSTD_BOUNDARY_RESEARCH
unset SAIL_VSTD_BOUNDARY_LOCAL SAIL_SHIPPED_RECORD SAIL_SHIPPED_DEPS SAIL_RETAINED_CONTENT_RESEARCH
if command -v cargo-verus >/dev/null; then
  echo "release-smoke: remove cargo-verus from PATH; the archive must work without it" >&2
  exit 1
fi
# A short path: the job socket lives under SAIL_CACHE_DIR, and macOS's
# default TMPDIR makes it longer than a Unix socket path allows (SAIL0810).
work="$(mktemp -d /tmp/sail-smoke.XXXXXX)"
echo "release-smoke: working in $work"

(cd "$(dirname "$archive")" && shasum -a 256 -c "$(basename "$archive").sha256")
release="$("$release_python" -B "$release_helper" unpack "$archive" --destination "$work")"
sail="$release/bin/sail"
export SAIL_CACHE_DIR="$work/cache"
"$sail" --version

status="$("$sail" engine status --output json)"
case "$status" in
  *"\"build\":\"$release/engine/"*) echo "verus: bundled runtime selected" ;;
  *) echo "release-smoke: sail did not select the bundled runtime: $status" >&2; exit 1 ;;
esac

# The README's new-project steps (First verification).
cd "$work"
"$sail" new --lib demo
cd demo
"$sail" verify
echo "verify: ok"

# A function to repair: the project's own lib.rs, with real specifications.
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
echo "verify (edited): ok"

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
echo "release-smoke: pass ($work)"
