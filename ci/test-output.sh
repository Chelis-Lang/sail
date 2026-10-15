#!/usr/bin/env bash
# The e2e `output` phase (ADR 0046): `sail verify`'s output against plain
# stock `cargo verus verify`, byte for byte, across the edit loop and the
# flags that change Verus's or Cargo's output. Run by `ci/run.sh e2e output`
# inside the CI image, with Sail's Verus selected (SAIL_VERUS_BUILD) and
# stock `cargo verus` on PATH.
#
#   ci/test-output.sh SAIL
set -euo pipefail
sail="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
export SAIL_CACHE_DIR="$work/cache"
cd "$work"
cargo verus new --lib demo >/dev/null
cd demo
cat > src/lib.rs <<'RS'
use vstd::prelude::*;

verus! {

pub fn max(a: u32, b: u32) -> (r: u32)
    ensures r >= a, r >= b, r == a || r == b,
{ if a > b { a } else { b } }

pub fn bad(x: u32) -> (r: u32)
    ensures r > x,
{ x }

} // verus!
RS
run() { # $1 = label; rest = command
  local label="$1"; shift
  touch src/lib.rs
  set +e
  "$@" > "$work/$label.out" 2> "$work/$label.err"
  echo $? > "$work/$label.rc"
  set -e
}
# Stock keeps its own Cargo target directory; managed `sail verify` builds in
# its own target under the Sail cache whatever CARGO_TARGET_DIR says, and the
# variable reaches only setups it verifies as plain Cargo-Verus does.
plain_run() { CARGO_TARGET_DIR="$work/target-plain" run "$@"; }
sail_run() { CARGO_TARGET_DIR="$work/target-sail" run "$@"; }
# Warm-ups: each side builds and verifies vstd once.
plain_run warmup-plain cargo verus verify
sail_run warmup-sail "$sail" verify
# The same comparison for the rest of the edit loop (ADR 0046). Each
# case runs plain, then Sail, on the same source; `norm` blanks Cargo's
# `Finished ... in Ns` timing, which differs between any two runs.
norm() { sed -E 's/(Finished.* in )[0-9.]+s$/\1Ns/' "$1"; }
same() { # plain-label sail-label [parts]
  local p="$1" s="$2" part
  for part in ${3:-rc out err}; do
    if ! cmp -s <(norm "$work/$p.$part") <(norm "$work/$s.$part"); then
      echo "e2e: $s $part differs from plain cargo verus verify ($p)" >&2
      diff -u <(norm "$work/$p.$part") <(norm "$work/$s.$part") >&2 || true
      [ "$part" = err ] || cat "$work/$s.err" >&2
      exit 1
    fi
  done
}
quiet_run() { # like run, without touching the source
  local label="$1"; shift
  set +e
  "$@" > "$work/$label.out" 2> "$work/$label.err"
  echo $? > "$work/$label.rc"
  set -e
}
# Combined output (2>&1), as agents read it, on the failing proof.
set +e
touch src/lib.rs
CARGO_TARGET_DIR="$work/target-plain" cargo verus verify > "$work/combined-plain.out" 2>&1
touch src/lib.rs
CARGO_TARGET_DIR="$work/target-sail" "$sail" verify > "$work/combined-sail.out" 2>&1
set -e
same combined-plain combined-sail out
# Passing proofs, then an unchanged re-run (Cargo's no-op; Sail reuses
# its receipts), then a compile error.
sed -i 's/ensures r > x,/ensures r >= x,/' src/lib.rs
plain_run pass-plain cargo verus verify
sail_run pass-sail "$sail" verify
same pass-plain pass-sail
grep -q "verified, 0 errors" "$work/pass-plain.out"
CARGO_TARGET_DIR="$work/target-plain" quiet_run noop-plain cargo verus verify
CARGO_TARGET_DIR="$work/target-sail" quiet_run noop-sail "$sail" verify
same noop-plain noop-sail
printf 'pub fn broken() -> u32 { y }\n' >> src/lib.rs
plain_run compile-plain cargo verus verify
sail_run compile-sail "$sail" verify
same compile-plain compile-sail
grep -q "cannot find value" "$work/compile-plain.err"
sed -i '/pub fn broken/d' src/lib.rs
# Verus's own stdout flags. New Verus arguments are a new context for
# Sail (its first run verifies vstd again), so each side runs once
# first. --time: the table's figures and build lines (Sail's Verus is
# built from the fork, ADR 0043) are blanked.
edit_run() { printf '// %s\n' "$1" >> src/lib.rs; quiet_run "$@"; }
CARGO_TARGET_DIR="$work/target-plain" edit_run time-plainwarm cargo verus verify -- --time
CARGO_TARGET_DIR="$work/target-sail" edit_run time-sailwarm "$sail" verify -- --time
CARGO_TARGET_DIR="$work/target-plain" edit_run outjson-plainwarm cargo verus verify -- --output-json
CARGO_TARGET_DIR="$work/target-sail" edit_run outjson-sailwarm "$sail" verify -- --output-json
CARGO_TARGET_DIR="$work/target-plain" edit_run time-plain cargo verus verify -- --time
CARGO_TARGET_DIR="$work/target-sail" edit_run time-sail "$sail" verify -- --time
same time-plain time-sail "rc err"
timing() { sed -E '/^  (Version|Toolchain):/d; s/[0-9]+/N/g; s/ +/ /g' "$1"; }
if ! cmp -s <(timing "$work/time-plain.out") <(timing "$work/time-sail.out"); then
  echo "e2e: -- --time stdout differs from plain cargo verus verify" >&2
  diff -u <(timing "$work/time-plain.out") <(timing "$work/time-sail.out") >&2 || true
  exit 1
fi
CARGO_TARGET_DIR="$work/target-plain" edit_run outjson-plain cargo verus verify -- --output-json
CARGO_TARGET_DIR="$work/target-sail" edit_run outjson-sail "$sail" verify -- --output-json
same outjson-plain outjson-sail "rc err"
python3 - "$work/outjson-plain.out" "$work/outjson-sail.out" <<'PY'
import json, sys
docs = [json.loads(open(path).read()) for path in sys.argv[1:]]
for key in ("verification-results", "func-details"):
    assert docs[0][key] == docs[1][key], (key, docs[0][key], docs[1][key])
assert "times-ms" not in docs[1], "times-ms without --time"
PY
# A workspace: two crates, `-p` and a feature, each side warmed first.
ws="$work/ws"
mkdir -p "$ws/a/src" "$ws/b/src"
vstd_dep="$(grep '^vstd' Cargo.toml)"
printf '[workspace]\nmembers = ["a", "b"]\nresolver = "2"\n' > "$ws/Cargo.toml"
printf '[package]\nname = "a"\nversion = "0.1.0"\nedition = "2021"\n\n[dependencies]\n%s\n\n[package.metadata.verus]\nverify = true\n' "$vstd_dep" > "$ws/a/Cargo.toml"
printf '[package]\nname = "b"\nversion = "0.1.0"\nedition = "2021"\n\n[features]\nextra = []\n\n[dependencies]\n%s\na = { path = "../a" }\n\n[package.metadata.verus]\nverify = true\n' "$vstd_dep" > "$ws/b/Cargo.toml"
cat > "$ws/a/src/lib.rs" <<'RS'
use vstd::prelude::*;

verus! {

pub fn double(x: u32) -> (r: u32)
    requires x < 1000,
    ensures r == 2 * x,
{ x + x }

} // verus!
RS
cat > "$ws/b/src/lib.rs" <<'RS'
use vstd::prelude::*;

verus! {

pub fn quad(x: u32) -> (r: u32)
    requires x < 100,
    ensures r == 4 * x,
{ a::double(a::double(x)) }

#[cfg(feature = "extra")]
pub fn wrong(x: u32) -> (r: u32)
    ensures r == x + 1,
{ x }

} // verus!
RS
(
  cd "$ws"
  ws_run() { printf '// %s\n' "$1" >> b/src/lib.rs; quiet_run "$@"; }
  CARGO_TARGET_DIR="$work/target-ws-plain" ws_run ws-plainwarm cargo verus verify -p b --features extra
  CARGO_TARGET_DIR="$work/target-ws-plain" ws_run ws-plain cargo verus verify -p b --features extra
  ws_run ws-sailwarm "$sail" verify -p b --features extra
  ws_run ws-sail "$sail" verify -p b --features extra
  same ws-plain ws-sail
  grep -q "postcondition not satisfied" "$work/ws-plain.err"
)
# Partial verification under `verify`: plain Cargo-Verus's own refusal.
plain_run partial-plain cargo verus verify -- --verify-function max
sail_run partial-sail "$sail" verify -- --verify-function max
same partial-plain partial-sail
grep -q "Partial verification must use" "$work/partial-plain.err"
echo "e2e: combined output, passing, unchanged, compile error, --time, --output-json, partial verification and a -p/--features workspace identical to cargo verus verify"
# A process-loader variable: verified as plain Cargo-Verus verifies it, with
# one note line, no proof receipts and no dependency store. The failing proof
# is back for this case; the JSON run builds the loader context's own target.
sed -i 's/ensures r >= x,/ensures r > x,/' src/lib.rs
LD_LIBRARY_PATH="$work/no-libraries" CARGO_TARGET_DIR="$work/target-sail" \
  quiet_run loader-json "$sail" verify --output json
grep -q '^note\[SAIL3112\]: LD_LIBRARY_PATH is set' "$work/loader-json.err"
python3 - "$work/loader-json.out" "$SAIL_CACHE_DIR" <<'PY'
import json, pathlib, sys
doc = json.loads(open(sys.argv[1]).read())
assert doc["error"] is None and doc["currentness"] == "unknown", doc["currentness"]
assert any(n.startswith("SAIL3112:") for n in doc["notes"]), doc["notes"]
assert not any(t["reused"] for t in doc["targets"]), "no reuse"
contexts = list(pathlib.Path(sys.argv[2], "projects").glob("*/" + doc["context_sha256"]))
assert len(contexts) == 1 and not list((contexts[0] / "receipts").glob("*.json")), contexts
PY
plain_run loader-plain cargo verus verify
LD_LIBRARY_PATH="$work/no-libraries" CARGO_TARGET_DIR="$work/target-sail" \
  run loader-sail "$sail" verify
[ "$(grep -c '^note\[SAIL3112\]' "$work/loader-sail.err")" = 1 ]
sed -i '/^note\[SAIL3112\]/d' "$work/loader-sail.err"
same loader-plain loader-sail
echo "e2e: LD_LIBRARY_PATH verified as plain cargo verus verify, without proof reuse"

# Setups managed verification cannot bind to a proof receipt (ADR 0046)
# are verified as plain Cargo-Verus verifies them: one note line, then
# Cargo-Verus's own output and exit code. Each side runs once to warm
# its target; the second runs are compared, with Cargo's `Finished ... in
# Ns` timing blanked.
unbound_normalize() { sed -E '/^note\[SAIL3112\]/d; s/(Finished.* in )[0-9.]+s$/\1Ns/' "$1"; }
unbound_compare() { # label
  local label="$1" part
  if [ "$(grep -c '^note\[SAIL3112\]' "$work/$label-sail.err")" != 1 ]; then
    echo "e2e: $label: expected one note[SAIL3112] line" >&2
    cat "$work/$label-sail.err" >&2
    exit 1
  fi
  for part in rc out err; do
    if ! cmp -s <(unbound_normalize "$work/$label-plain.$part") <(unbound_normalize "$work/$label-sail.$part"); then
      echo "e2e: $label $part differs from plain cargo verus verify" >&2
      diff -u <(unbound_normalize "$work/$label-plain.$part") <(unbound_normalize "$work/$label-sail.$part") >&2 || true
      exit 1
    fi
  done
}
unbound_case() { # label; rest = the arguments for both sides
  local label="$1"; shift
  plain_run "$label-plainwarm" cargo verus verify "$@"
  plain_run "$label-plain" cargo verus verify "$@"
  sail_run "$label-sailwarm" "$sail" verify "$@"
  sail_run "$label-sail" "$sail" verify "$@"
  unbound_compare "$label"
}
unbound_case config --config build.jobs=2
unbound_case tests --tests
# An explicit target directory: each side its own, already built by the
# cases above (a fresh one would add two cold builds to this job).
run target-dir-plain cargo verus verify --target-dir "$work/target-plain"
run target-dir-sail "$sail" verify --target-dir "$work/target-sail"
unbound_compare target-dir
# Last: plain Cargo-Verus then treats the crate as fresh for any later
# Verus arguments, since `--no-verify` builds it successfully.
unbound_case no-verify -- --no-verify
echo "e2e: --no-verify, --config, --tests and --target-dir verified as plain cargo verus verify"

