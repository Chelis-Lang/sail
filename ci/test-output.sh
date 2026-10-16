#!/usr/bin/env bash
# The e2e `output` phase (ADR 0046): `sail verify`'s output against plain
# stock `cargo verus verify`, byte for byte, across the edit loop and the
# flags that change Verus's or Cargo's output. Run by `ci/run.sh e2e output`
# inside the CI image, with Sail's Verus selected (SAIL_VERUS_BUILD) and
# stock `cargo verus` on PATH.
#
#   ci/test-output.sh SAIL loop|loader|flags|extra
#
# `loop`: combined output, passing proofs, an unchanged re-run, a compile
# error and partial verification. `loader`: a loader variable. `flags`: the
# setups verified without proof reuse. `extra`: `-- --output-json --time` and
# a `-p`/`--features` workspace. Every cold build verifies vstd (one to two
# minutes on a two-CPU CI runner), so CI runs `loop`, `loader` and `flags` as
# separate jobs with two each, run at once; `extra`, which needs six, is
# local-only (`ci/local.sh e2e extra`).
set -euo pipefail
sail="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
part="${2:?usage: ci/test-output.sh SAIL loop|loader|flags|extra}"
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
edit_run() { printf '// %s\n' "$1" >> src/lib.rs; quiet_run "$@"; }
# Cold warm-ups of both sides at once. Each builds and verifies vstd in its
# own target (and Sail in its own cache), and their output is never
# compared, so they need not run one after the other: on a two-CPU runner
# a cold build leaves the other CPU partly idle. One resolve first, so the
# two never write the lockfile together.
cargo generate-lockfile --quiet
touch src/lib.rs
warm_plain() { CARGO_TARGET_DIR="$work/target-plain" quiet_run "$@"; }
warm_sail() { CARGO_TARGET_DIR="$work/target-sail" quiet_run "$@"; }
# Warm-ups: each side builds and verifies vstd once, where the part needs it.
# The loader part's Sail run builds its own context meanwhile (below).
case "$part" in loop | loader | extra) warm_plain warmup-plain cargo verus verify & ;; esac
case "$part" in loop | extra) warm_sail warmup-sail "$sail" verify & ;; esac
[ "$part" = loader ] || wait

if [ "$part" = loop ]; then
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
# Partial verification under `verify`: plain Cargo-Verus's own refusal.
plain_run partial-plain cargo verus verify -- --verify-function max
sail_run partial-sail "$sail" verify -- --verify-function max
same partial-plain partial-sail
grep -q "Partial verification must use" "$work/partial-plain.err"
echo "e2e: combined output, passing, unchanged, compile error and partial verification identical to cargo verus verify"

fi

if [ "$part" = flags ]; then
# Setups managed verification cannot bind to a proof receipt (ADR 0046)
# are verified as plain Cargo-Verus verifies them: one note line, then
# Cargo-Verus's own output and exit code. Each side runs once (both at once) to warm
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
  touch src/lib.rs
  warm_plain "$label-plainwarm" cargo verus verify "$@" &
  warm_sail "$label-sailwarm" "$sail" verify "$@" &
  wait
  plain_run "$label-plain" cargo verus verify "$@"
  sail_run "$label-sail" "$sail" verify "$@"
  unbound_compare "$label"
}
unbound_case config --config build.jobs=2
unbound_case tests --tests
# The same setup as a background job, as jobs, `watch` and MCP run it
# (#165): verified as plain Cargo-Verus verifies it, in its own target, and
# reported as one project result marked `no_reuse`, never current.
touch src/lib.rs
job="$(CARGO_TARGET_DIR="$work/target-sail" "$sail" verify --background --output json --tests \
  | python3 -c 'import json, sys; print(json.load(sys.stdin)["id"])')"
for _ in $(seq 1500); do
  "$sail" jobs result "$job" --output json > "$work/tests-job.json"
  python3 -c 'import json, sys; sys.exit(json.load(open(sys.argv[1]))["value"]["job"]["state"] in ("queued", "running"))' \
    "$work/tests-job.json" && break
  sleep 0.2
done
python3 - "$work/tests-job.json" "$(cat "$work/tests-plain.rc")" <<'PY'
import json, sys
value = json.load(open(sys.argv[1]))["value"]
job, result = value["job"], value["result"]
assert job["state"] == "completed", job
assert job["currentness"] == "unknown", job
assert result["no_reuse"]["code"] == "SAIL3112", result.get("no_reuse")
assert result["no_reuse"]["verified_as"] == "plain-cargo-verus", result["no_reuse"]
assert "--tests" in result["no_reuse"]["reason"], result["no_reuse"]
assert result["currentness"] == "unknown" and result["exit_code"] == int(sys.argv[2]), result["exit_code"]
[target] = result["targets"]
assert target["requested"] and not target["reused"], target
failed = [f["function"] for c in target["report"]["crates"] for f in c["functions"] if f["verdict"] == "failed"]
assert any(name.endswith("bad") for name in failed), failed
PY
echo "e2e: a --tests job verified as plain cargo verus verify, without proof reuse"
# An explicit target directory: each side its own, already built by the
# cases above (a fresh one would add two cold builds to this job).
run target-dir-plain cargo verus verify --target-dir "$work/target-plain"
run target-dir-sail "$sail" verify --target-dir "$work/target-sail"
unbound_compare target-dir
# Last: plain Cargo-Verus then treats the crate as fresh for any later
# Verus arguments, since `--no-verify` builds it successfully.
unbound_case no-verify -- --no-verify
echo "e2e: --no-verify, --config, --tests and --target-dir verified as plain cargo verus verify"
fi

if [ "$part" = loader ]; then
# A process-loader variable: verified as plain Cargo-Verus verifies it, with
# one note line, no proof receipts and no dependency store. The failing proof
# fails; the JSON run builds the loader context's own target.
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
wait # the plain warm-up
plain_run loader-plain cargo verus verify
LD_LIBRARY_PATH="$work/no-libraries" CARGO_TARGET_DIR="$work/target-sail" \
  run loader-sail "$sail" verify
[ "$(grep -c '^note\[SAIL3112\]' "$work/loader-sail.err")" = 1 ]
sed -i '/^note\[SAIL3112\]/d' "$work/loader-sail.err"
same loader-plain loader-sail
echo "e2e: LD_LIBRARY_PATH verified as plain cargo verus verify, without proof reuse"
fi

if [ "$part" = extra ]; then
# Verus's own stdout flags, as the conformance harness passes them. New
# Verus arguments are a new context for Sail (its first run verifies vstd
# again), so each side runs once first. The documents' timings and build
# information (Sail's Verus is built from the fork, ADR 0043) differ; their
# results must not.
CARGO_TARGET_DIR="$work/target-plain" edit_run json-plainwarm cargo verus verify -- --output-json --time
CARGO_TARGET_DIR="$work/target-sail" edit_run json-sailwarm "$sail" verify -- --output-json --time
CARGO_TARGET_DIR="$work/target-plain" edit_run json-plain cargo verus verify -- --output-json --time
CARGO_TARGET_DIR="$work/target-sail" edit_run json-sail "$sail" verify -- --output-json --time
same json-plain json-sail "rc err"
python3 - "$work/json-plain.out" "$work/json-sail.out" <<'PY'
import json, sys
docs = [json.loads(open(path).read()) for path in sys.argv[1:]]
for key in ("verification-results", "func-details"):
    assert docs[0][key] == docs[1][key], (key, docs[0][key], docs[1][key])
assert all("times-ms" in d for d in docs), "times-ms with --time"
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
echo "e2e: --output-json --time and a -p/--features workspace identical to cargo verus verify"
fi
