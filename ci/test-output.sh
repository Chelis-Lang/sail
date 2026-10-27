#!/usr/bin/env bash
# The e2e `output` phase (ADR 0046): `sail verify`'s output against plain
# stock `cargo verus verify` under the toolchain Sail selects for its Cargo
# (`RUSTUP_TOOLCHAIN`, ADR 0058), byte for byte, across the edit loop and the
# flags that change Verus's or Cargo's output. Run by `ci/run.sh e2e output`
# inside the CI image, with Sail's Verus selected (SAIL_VERUS_BUILD) and
# stock `cargo verus` on PATH.
#
#   ci/test-output.sh SAIL loop [noop]|loader|flags|extra|cargo-range
#
# `loop`: combined output, passing proofs, an unchanged re-run, a compile
# error, partial verification and a seeded-mutation smoke; with `noop`
# (`ci/run.sh` always passes it), then unchanged-call replay (ADR 0082) and
# every change that must miss it, on the same warm targets. `loader`: a loader
# variable. `flags`: the
# setups verified without proof reuse. `extra`: `-- --output-json --time` and
# a `-p`/`--features` workspace. `cargo-range` (local-only): projects whose
# rust-toolchain.toml selects another qualified Cargo (ADR 0084), named by
# SAIL_CARGO_RANGE_E2E, and an unqualified one (SAIL_CARGO_RANGE_E2E_UNQUALIFIED);
# each must be installed. Every cold build verifies vstd (one to two
# minutes on a two-CPU CI runner), so CI runs `loop`, `loader` and `flags` as
# separate jobs with two each, run at once; `extra`, which needs six, is
# local-only (`ci/local.sh e2e extra`).
set -euo pipefail
sail="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
part="${2:?usage: ci/test-output.sh SAIL loop [noop]|loader|flags|extra|cargo-range}"
noop_part="${3:-}"
case "$part:$noop_part:$#" in
  loop::2 | loop:noop:3 | loader::2 | flags::2 | extra::2 | cargo-range::2) ;;
  *) echo 'usage: ci/test-output.sh SAIL loop [noop]|loader|flags|extra|cargo-range' >&2; exit 2 ;;
esac
sail_output_phase="$part"
sail_output_checkout="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
sail_output_evidence="${SAIL_E2E_OUTPUT_OUT:-$sail_output_checkout/target/e2e-output-evidence}"
# Resolve a caller-relative observer path before entering the temporary fixture.
case "$sail_output_evidence" in /*) ;; *) sail_output_evidence="$PWD/$sail_output_evidence" ;; esac
# The observer path is harness state, never a verifier input.
unset SAIL_E2E_OUTPUT_OUT
# The per-function verdict checker (ci/test-output-evidence.py substitutes one).
smoke_check="${SAIL_E2E_CHECK_OUTPUT:-$sail_output_checkout/target/release/examples/check_output}"
unset SAIL_E2E_CHECK_OUTPUT
work="$(mktemp -d)"
# Stock macOS has neither GNU sed's `-i` spelling nor `timeout`. Edit through
# a copy (keeping the file's inode and mode, as `sed -i` keeps its mode), and
# use coreutils' timeout where installed, else ci/timeout.py.
edit_in_place() {
  sed "$1" "$2" > "$2.sail-edit"
  cat "$2.sail-edit" > "$2"
  rm "$2.sail-edit"
}
bounded() {
  if command -v timeout > /dev/null; then
    timeout "$@"
  else
    python3 "$sail_output_checkout/ci/timeout.py" "$@"
  fi
}
output_cleanup() {
  local original_exit=$? capture_exit
  trap - EXIT
  set +e
  if [ "$original_exit" -ne 0 ]; then
    bounded --signal=TERM --kill-after=5s 60s python3 \
      "$sail_output_checkout/ci/capture-output-evidence.py" \
      --work "$work" --out "$sail_output_evidence" --part "$sail_output_phase" \
      --exit "$original_exit" --sail "$sail"
    capture_exit=$?
    if [ "$capture_exit" -ne 0 ]; then
      echo "e2e: failure evidence collector exited $capture_exit; original test exit $original_exit retained" >&2
    fi
  fi
  rm -rf "$work"
  exit "$original_exit"
}
trap output_cleanup EXIT
# These copies are outside the project and are diagnostic observations only.
# Failure to record metadata never changes a verifier command or its exit code.
record_start() {
  local label="$1" sail_output_source record_failed=0; shift
  mkdir -p "$work/commands" || record_failed=1
  printf '%s\0' "$@" > "$work/commands/$label.argv" || record_failed=1
  printf '%s\n' "$PWD" > "$work/commands/$label.cwd" || record_failed=1
  printf '%s\n' "${CARGO_TARGET_DIR-}" > "$work/commands/$label.target-dir" || record_failed=1
  date -u +%s > "$work/commands/$label.started" || record_failed=1
  python3 "$sail_output_checkout/ci/capture-output-evidence.py" snapshot \
    --work "$work" --cwd "$PWD" --label "$label" --stage start || record_failed=1
  if [ "$record_failed" -ne 0 ]; then
    echo "e2e: could not record command metadata for $label" >&2
    printf '%s\n' "$label metadata recording failed" >> "$work/metadata-errors.err" || true
  fi
}
record_finish() {
  local label="$1" code="$2" sail_output_stream record_failed=0
  python3 "$sail_output_checkout/ci/capture-output-evidence.py" snapshot \
    --work "$work" --cwd "$PWD" --label "$label" --stage finish || record_failed=1
  printf '%s\n' "$code" > "$work/$label.rc" || record_failed=1
  date -u +%s > "$work/commands/$label.finished" || record_failed=1
  if [ "$record_failed" -ne 0 ]; then
    echo "e2e: could not record command completion for $label" >&2
    printf '%s\n' "$label completion recording failed" >> "$work/metadata-errors.err" || true
  fi
}
export SAIL_CACHE_DIR="$work/cache"
# Stock runs with the toolchain Sail selects for its Cargo (ADR 0058).
sail_stock_toolchain="$(python3 "$sail_output_checkout/ci/stock-toolchain.py" "$sail")"
stock_cargo() { RUSTUP_TOOLCHAIN="$sail_stock_toolchain" cargo "$@"; }
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
# The original, restored before the replay cases.
cp src/lib.rs "$work/demo-lib.rs"
run() { # $1 = label; rest = command
  local label="$1" code; shift
  touch src/lib.rs
  record_start "$label" "$@"
  set +e
  "$@" > "$work/$label.out" 2> "$work/$label.err"
  code=$?
  record_finish "$label" "$code"
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
  local label="$1" code; shift
  record_start "$label" "$@"
  set +e
  "$@" > "$work/$label.out" 2> "$work/$label.err"
  code=$?
  record_finish "$label" "$code"
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
case "$part" in loop | loader | extra) warm_plain warmup-plain stock_cargo verus verify & ;; esac
case "$part" in loop | extra) warm_sail warmup-sail "$sail" verify & ;; esac
[ "$part" = loader ] || wait

if [ "$part" = loop ]; then
# Combined output (2>&1), as agents read it, on the failing proof.
set +e
touch src/lib.rs
CARGO_TARGET_DIR="$work/target-plain" record_start combined-plain stock_cargo verus verify
CARGO_TARGET_DIR="$work/target-plain" stock_cargo verus verify > "$work/combined-plain.out" 2>&1
sail_output_exit=$?
record_finish combined-plain "$sail_output_exit"
touch src/lib.rs
CARGO_TARGET_DIR="$work/target-sail" record_start combined-sail "$sail" verify
CARGO_TARGET_DIR="$work/target-sail" "$sail" verify > "$work/combined-sail.out" 2>&1
sail_output_exit=$?
record_finish combined-sail "$sail_output_exit"
set -e
same combined-plain combined-sail out
# Passing proofs, then an unchanged re-run (Cargo's no-op; Sail reuses
# its receipts), then a compile error.
edit_in_place 's/ensures r > x,/ensures r >= x,/' src/lib.rs
plain_run pass-plain stock_cargo verus verify
sail_run pass-sail "$sail" verify
same pass-plain pass-sail
grep -q "verified, 0 errors" "$work/pass-plain.out"
CARGO_TARGET_DIR="$work/target-plain" quiet_run noop-plain stock_cargo verus verify
CARGO_TARGET_DIR="$work/target-sail" quiet_run noop-sail "$sail" verify
same noop-plain noop-sail
printf 'pub fn broken() -> u32 { y }\n' >> src/lib.rs
plain_run compile-plain stock_cargo verus verify
sail_run compile-sail "$sail" verify
same compile-plain compile-sail
grep -q "cannot find value" "$work/compile-plain.err"
edit_in_place '/pub fn broken/d' src/lib.rs
# Partial verification under `verify`: plain Cargo-Verus's own refusal.
plain_run partial-plain stock_cargo verus verify -- --verify-function max
sail_run partial-sail "$sail" verify -- --verify-function max
same partial-plain partial-sail
grep -q "Partial verification must use" "$work/partial-plain.err"
# An extension impl without a matching standard library impl: Verus labels
# core's impl from vstd's VIR, cannot resolve that span and prints its stored
# text with `dbg!`, location included (VI-0031). Both must read as stock's.
cp src/lib.rs "$work/lib.rs.keep"
cat > src/lib.rs <<'RS'
use vstd::prelude::*;
use vstd::std_specs::fmt::DisplaySpecImpl;

verus! {

pub struct S;

impl core::fmt::Display for S {
    #[verifier::external_body]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Ok(())
    }
}

impl<'a> DisplaySpecImpl for &'a S {}

} // verus!
RS
plain_run fallback-plain stock_cargo verus verify
sail_run fallback-sail "$sail" verify
same fallback-plain fallback-sail
grep -q '^\[rust_verify/src/verifier.rs:[0-9]*:[0-9]*\] &sp.as_string = ' "$work/fallback-plain.err"
cp "$work/lib.rs.keep" src/lib.rs
# A log written into the workspace during a run (#259): that run cannot be
# current, but the next unchanged run must still be Cargo's own no-op, as
# plain Cargo-Verus's is, without re-running the registry build scripts.
CARGO_TARGET_DIR="$work/target-plain" stock_cargo verus verify > in-workspace.log 2>&1
CARGO_TARGET_DIR="$work/target-sail" "$sail" verify > in-workspace.log 2>&1
CARGO_TARGET_DIR="$work/target-plain" quiet_run logged-plain stock_cargo verus verify
CARGO_TARGET_DIR="$work/target-sail" quiet_run logged-sail "$sail" verify
same logged-plain logged-sail
if grep -qE '^ *(Compiling|Checking) ' "$work/logged-sail.err"; then
  echo "e2e: the run after a workspace log compiled again" >&2
  exit 1
fi
rm in-workspace.log
echo "e2e: combined output, passing, unchanged, compile error, partial verification, the stored-span fallback and the run after a workspace log identical to cargo verus verify"

# A seeded-mutation smoke (ADR 0063) through managed `sail verify`: the
# original, an unchanged re-run, seeded mutants from `cargo xtask gen-mutants`
# (ci/fixtures/mutation-smoke/smoke.toml), each followed by a revert to the
# original, and a forced re-run. Each step is compared with stock as above
# (`same`); then an unchanged re-run of each side, Sail's with `--output
# json`, whose per-function verdicts must be stock's `--output-json` ones
# (check_output). Every revert and the forced run must get the original's
# answers. The catalogue's `expected` is stock's outcome.
smoke_start=$SECONDS
smoke_dir="$sail_output_checkout/ci/fixtures/mutation-smoke"
smoke_mutants="smoke-compare-001 smoke-logic-001 smoke-statement-001 smoke-constant-001"
cp "$smoke_dir/lib.rs" src/lib.rs
smoke_step() { # label; rest = environment for the Sail side
  local label="$1"; shift
  plain_run "$label-plain" stock_cargo verus verify
  sail_run "$label-sail" env "$@" "$sail" verify
  same "$label-plain" "$label-sail"
}
# Unchanged re-runs: stock's human one and Sail's `--output json`, which
# must have the same exit code and stderr, and stock's verdicts.
smoke_json() { # label
  CARGO_TARGET_DIR="$work/target-plain" quiet_run "$1-again" stock_cargo verus verify
  CARGO_TARGET_DIR="$work/target-sail" quiet_run "$1-json" "$sail" verify --output json
  same "$1-again" "$1-json" "rc err"
}
smoke_verdicts() { # label verus-json-label
  "$smoke_check" "$work/$1-json.out" "$work/$2.out" > /dev/null \
    || { echo "e2e: $1 per-function verdicts differ from stock's ($2)" >&2; exit 1; }
}
# Stock's `--output-json` results for the current source. Changed Verus
# arguments alone do not re-verify a fresh crate (VI-0005), so clean it first.
smoke_verus_json() { # label
  CARGO_TARGET_DIR="$work/target-plain" stock_cargo clean -q -p demo
  plain_run "$1" stock_cargo verus verify -- --output-json --time
}
smoke_original() { # label; rest = environment: also the original's answers
  smoke_step "$@"
  same base-plain "$1-sail"
  smoke_json "$1"
  smoke_verdicts "$1" base-verusjson
}
smoke_step base
grep -q "verified, 0 errors" "$work/base-plain.out"
CARGO_TARGET_DIR="$work/target-plain" quiet_run unchanged-plain stock_cargo verus verify
CARGO_TARGET_DIR="$work/target-sail" quiet_run unchanged-sail "$sail" verify
same unchanged-plain unchanged-sail
smoke_json base
smoke_verus_json base-verusjson
smoke_verdicts base base-verusjson
for id in $smoke_mutants; do
  python3 - "$smoke_dir/smoke.toml" "$id" "$smoke_dir/lib.rs" src/lib.rs > "$work/$id.expected" <<'PY'
import json, re, sys
catalogue, mutant, original, out = sys.argv[1:]
# The generator's strings: one-line JSON or multi-line literal (no tomllib on
# macOS's Python 3.9).
edits = []
for block in open(catalogue).read().split("\n[[edit]]\n")[1:]:
    fields = re.finditer(r"^(\w+) = (?:'''\n(.*?)'''|(\"[^\n]*\"))$", block, re.M | re.S)
    edits.append({m[1]: json.loads(m[3]) if m[2] is None else m[2] for m in fields})
[edit] = [e for e in edits if e["id"] == mutant]
text = open(original).read()
assert text.count(edit["find"]) == 1, mutant
open(out, "w").write(text.replace(edit["find"], edit["replace"]))
print(edit["expected"])
PY
  expected="$(cat "$work/$id.expected")"
  smoke_step "$id"
  case "$expected:$(cat "$work/$id-plain.rc")" in
    verified:0) ;;
    errors:[1-9]*)
      grep -q "verification results:: .*, [1-9][0-9]* errors" "$work/$id-plain.out" || {
        echo "e2e: stock did not reach verification errors for $id" >&2
        cat "$work/$id-plain.err" >&2
        exit 1
      } ;;
    *)
      echo "e2e: stock's outcome for $id differs from the catalogue's $expected (exit $(cat "$work/$id-plain.rc"))" >&2
      exit 1 ;;
  esac
  smoke_json "$id"
  smoke_verus_json "$id-verusjson"
  smoke_verdicts "$id" "$id-verusjson"
  cp "$smoke_dir/lib.rs" src/lib.rs
  smoke_original "$id-revert"
done
# Fresh verification forced on both sides of the unchanged original.
CARGO_TARGET_DIR="$work/target-plain" stock_cargo clean -q -p demo
smoke_original forced SAIL_MANAGED_FORCE=1
echo "e2e: seeded mutants ($smoke_mutants), their reverts, unchanged and forced runs identical to cargo verus verify ($((SECONDS - smoke_start)) s)"

# The program the compiler sees (#385, #393). A procedural macro and
# `option_env!` read the compiler's arguments and environment; each proof
# holds only if Sail adds something plain Cargo-Verus does not give it. Both
# sides must reject all three, plain and with `--output json`.
cp Cargo.toml "$work/Cargo.toml.keep"
cp Cargo.lock "$work/Cargo.lock.keep"
cp src/lib.rs "$work/lib.rs.keep"
mkdir -p witness/src
cat > witness/Cargo.toml <<'TOML'
[package]
name = "witness"
version = "0.1.0"
edition = "2021"

[lib]
proc-macro = true
TOML
cat > witness/src/lib.rs <<'RS'
use proc_macro::TokenStream;

#[proc_macro]
pub fn witnesses(_: TokenStream) -> TokenStream {
    let argv = std::env::args().any(|a| {
        matches!(a.as_str(), "--time" | "--output-json" | "--log" | "--log-dir")
            || a.starts_with("-Zchecksum-hash-algorithm=")
    });
    let env = std::env::vars_os().any(|(k, _)| k.to_string_lossy().starts_with("SAIL_MANAGED_"));
    format!(
        "verus! {{ pub proof fn argv_witness() {{ assert({argv}); }} pub proof fn env_witness() {{ assert({env}); }} }}"
    )
    .parse()
    .unwrap()
}
RS
awk -v dep='witness = { path = "witness" }' '
  { print } $0 == "[dependencies]" { print dep; added = 1 }
  END { if (!added) { print "[dependencies]"; print dep } }' "$work/Cargo.toml.keep" > Cargo.toml
grep -q '^witness = ' Cargo.toml
cat > src/lib.rs <<'RS'
use vstd::prelude::*;

witness::witnesses!();

verus! {

proof fn injected_runtime_is_present() {
    assert(match option_env!("SAIL_MANAGED_RUNTIME") {
        Some(_) => true,
        None => false,
    });
}

} // verus!
RS
plain_run visible-plain stock_cargo verus verify
sail_run visible-sail "$sail" verify
same visible-plain visible-sail
grep -q "verification results:: 0 verified, 3 errors" "$work/visible-plain.out"
CARGO_TARGET_DIR="$work/target-plain" quiet_run visible-again stock_cargo verus verify
CARGO_TARGET_DIR="$work/target-sail" quiet_run visible-json "$sail" verify --output json
same visible-again visible-json "rc err"
cp "$work/Cargo.toml.keep" Cargo.toml
cp "$work/Cargo.lock.keep" Cargo.lock
cp "$work/lib.rs.keep" src/lib.rs
rm -r witness
echo "e2e: the compiler's arguments and environment as plain cargo verus verify gives them (#385, #393)"

fi

if [ "$noop_part" = noop ]; then
# Unchanged-call replay (ADR 0082). The first unchanged call after a
# verification takes the full path and records; an identical later call
# replays it without Cargo. Every change below must miss, and every output
# must still match plain Cargo-Verus. Each decision is logged to the trace,
# which is constant for the whole part (it is part of the replay key).
# The cases need warm stock and managed Sail targets, so they follow `loop`
# and reuse its warm-ups rather than a job of their own, starting from the
# original source.
noop_start=$SECONDS
cp "$work/demo-lib.rs" src/lib.rs
export SAIL_MANAGED_NOOP_TRACE="$work/noop.trace"
: > "$SAIL_MANAGED_NOOP_TRACE"
decision() { tail -n 1 "$SAIL_MANAGED_NOOP_TRACE"; }
expect() { # label pattern
  local seen
  seen="$(grep -E '^(hit|miss: )' "$SAIL_MANAGED_NOOP_TRACE" | tail -n 1)"
  if ! printf '%s\n' "$seen" | grep -qE "$2"; then
    echo "e2e: $1: expected replay decision /$2/, got '$seen'" >&2
    cat "$SAIL_MANAGED_NOOP_TRACE" >&2
    exit 1
  fi
}
recorded() { # label
  if ! decision | grep -q '^recorded: '; then
    echo "e2e: $1: the unchanged call was not recorded" >&2
    cat "$SAIL_MANAGED_NOOP_TRACE" >&2
    exit 1
  fi
}
# Recorded stamps must be at least 2 s older than the recording run.
settle() { sleep 3; }
pair() { # label: plain then Sail, without touching the source
  CARGO_TARGET_DIR="$work/target-plain" quiet_run "$1-plain" stock_cargo verus verify
  CARGO_TARGET_DIR="$work/target-sail" quiet_run "$1-sail" "$sail" verify
  same "$1-plain" "$1-sail"
}
# Record, then expect a replay that matches plain output.
rerecord() { # label
  settle
  pair "$1-record"
  recorded "$1-record"
  pair "$1-replay"
  expect "$1-replay" '^hit$'
}
edit_in_place 's/ensures r > x,/ensures r >= x,/' src/lib.rs
# A declared variable and an input outside the package, known only from
# dep-info, so each later case changes one of them.
printf 'first\n' > "$work/outside.txt"
cat >> src/lib.rs <<RS
pub const PROBE: Option<&str> = option_env!("NOOP_PROBE_VALUE");
pub const OUTSIDE: &str = include_str!("$work/outside.txt");
pub mod m;
RS
# A module, for an exact partial scope.
cat > src/m.rs <<'RS'
use vstd::prelude::*;

verus! {

pub fn one() -> (r: u32)
    ensures r == 1,
{ 1 }

} // verus!
RS
plain_run pass-plain stock_cargo verus verify
sail_run pass-sail "$sail" verify
same pass-plain pass-sail
# The first call after a newly discovered input outside the package is
# Cargo's no-op on both sides; it no longer rebuilds the build-script
# packages (#399).
pair discovered
if grep -qE '^ *(Compiling|Checking) ' "$work/discovered-sail.err"; then
  echo "e2e: the call after a discovered input rebuilt" >&2
  exit 1
fi
rerecord unchanged
# The same call again replays again.
pair unchanged-again
expect unchanged-again '^hit$'

# A content edit with its mtime restored still changes ctime.
cp -p src/lib.rs "$work/lib.rs.orig"
printf '// same size\n' >> src/lib.rs
touch -r "$work/lib.rs.orig" src/lib.rs
pair restored-mtime
expect restored-mtime '^miss: changed: .*src/lib.rs$'
rerecord after-restored-mtime
# A touch alone.
touch src/lib.rs
pair touched
expect touched '^miss: changed: .*src/lib.rs$'
rerecord after-touch
# A new and a removed unreferenced source file: replay misses, and the full
# path, like plain Cargo-Verus, verifies nothing (#398).
printf '// unused\n' > src/unused.rs
pair added-file
expect added-file '^miss: changed: '
rm src/unused.rs
pair removed-file
expect removed-file '^miss: '
for label in added-file removed-file; do
  if grep -qE '^ *(Compiling|Checking) ' "$work/$label-sail.err"; then
    echo "e2e: $label: an unreferenced file re-verified the crate" >&2
    exit 1
  fi
done
rerecord after-files
# A declared variable.
NOOP_PROBE_VALUE=set pair env-dep
expect env-dep '^miss: '
pair env-dep-unset
rerecord after-env-dep
# A dep-info-only input outside the package.
printf 'second\n' > "$work/outside.txt"
pair outside-input
expect outside-input '^miss: changed: .*outside.txt$'
rerecord after-outside-input
# Cargo.lock.
cp -p Cargo.lock "$work/Cargo.lock.orig"
printf '\n' >> Cargo.lock
pair lockfile
expect lockfile '^miss: changed: .*Cargo.lock$'
cp "$work/Cargo.lock.orig" Cargo.lock
pair lockfile-restored
expect lockfile-restored '^miss: '
rerecord after-lockfile
# Sail's own Cargo target loses the crate's fingerprints; plain Cargo-Verus's
# target loses the same, so both rebuild the crate.
find "$work/target-plain" "$SAIL_CACHE_DIR/projects" -type d -path '*/.fingerprint/demo-*' -prune -exec rm -rf {} +
pair fingerprints
expect fingerprints '^miss: '
rerecord after-fingerprints

# JSON output has its own record: its first call takes the full path.
settle
CARGO_TARGET_DIR="$work/target-sail" quiet_run json-record "$sail" verify --output json
expect json-record '^miss: no record$'
recorded json-record
CARGO_TARGET_DIR="$work/target-sail" quiet_run json-replay "$sail" verify --output json
expect json-replay '^hit$'
python3 - "$work/json-record.out" "$work/json-replay.out" <<'PY'
import json, sys
recorded, replayed = (json.load(open(path)) for path in sys.argv[1:])
assert replayed["currentness"] == "current" and replayed["exit_code"] == 0, replayed
assert replayed["run_id"] != recorded["run_id"], "a replay has its own run"
for doc in (recorded, replayed):
    doc.pop("run_id")
    doc["execution"].pop("wall_ms")
assert recorded == replayed, (recorded, replayed)
PY
# An exact partial scope replays only itself and is never promoted: the
# whole-target call after it has no record of its own and stays complete.
rm -f "$SAIL_CACHE_DIR"/noop/*.json
# A new scope builds its own target. Its first call discovers the input
# outside the package, so it is Historical and records no receipt; the second
# verifies the crate again to establish one; the third records.
for attempt in 1 2 3; do
  settle
  CARGO_TARGET_DIR="$work/target-sail" quiet_run partial-record "$sail" verify --output json -- --verify-module m
  expect partial-record '^miss: no record$'
  if decision | grep -q '^recorded: '; then break; fi
done
recorded partial-record
CARGO_TARGET_DIR="$work/target-sail" quiet_run partial-replay "$sail" verify --output json -- --verify-module m
expect partial-replay '^hit$'
CARGO_TARGET_DIR="$work/target-sail" quiet_run whole-after-partial "$sail" verify --output json
expect whole-after-partial '^miss: no record$'
python3 - "$work/partial-replay.out" "$work/whole-after-partial.out" <<'PY'
import json, sys
partial, whole = (json.load(open(path)) for path in sys.argv[1:])
assert any("partial" in note for note in partial["notes"]), partial["notes"]
[requested] = [t for t in partial["targets"] if t["requested"]]
assert not requested["complete"] and requested.get("scope_reuse"), requested
[requested] = [t for t in whole["targets"] if t["requested"]]
assert requested["complete"] and not requested.get("scope_reuse"), requested
PY
# The focused fail -> fix loop (#441): the passing call after a failing one
# rebuilds only what plain `cargo verus focus` rebuilds, never the dependency
# closure behind the build-script packages.
focus_pair() { # label
  CARGO_TARGET_DIR="$work/target-plain" quiet_run "$1-plain" stock_cargo verus focus -- --verify-module m
  CARGO_TARGET_DIR="$work/target-sail" quiet_run "$1-sail" "$sail" verify --output json -- --verify-module m
}
units() { grep -E '^ *(Compiling|Checking) ' "$work/$1.err" | sed -E 's/^ +//' | sort; }
focus_pair focus-warm
cp src/m.rs "$work/m.rs.orig"
edit_in_place 's/ensures r == 1,/ensures r == 2,/' src/m.rs
focus_pair focus-fail
[ "$(cat "$work/focus-fail-plain.rc")" != 0 ] && [ "$(cat "$work/focus-fail-sail.rc")" != 0 ]
cp "$work/m.rs.orig" src/m.rs
focus_pair focus-fix
[ "$(cat "$work/focus-fix-plain.rc")" = 0 ] && [ "$(cat "$work/focus-fix-sail.rc")" = 0 ]
if ! diff <(units focus-fix-plain) <(units focus-fix-sail) >&2 \
  || units focus-fix-sail | grep -v ' demo v' >&2; then
  echo "e2e: the passing focused call after a failing one rebuilt more than plain cargo verus focus" >&2
  exit 1
fi
# A failing project re-verifies on every unchanged call, as plain does.
edit_in_place 's/ensures r >= x,/ensures r > x,/' src/lib.rs
plain_run failing-plain stock_cargo verus verify
sail_run failing-sail "$sail" verify
same failing-plain failing-sail
settle
pair failing-unchanged
pair failing-again
if grep -q '^hit$' <(tail -n 4 "$SAIL_MANAGED_NOOP_TRACE"); then
  echo "e2e: a failing project was replayed" >&2
  exit 1
fi
# A new binary target, then its removal. Each verifies only what plain
# Cargo-Verus verifies: the binary, then nothing. Later unchanged calls are
# Cargo's no-op again and replay (#401).
edit_in_place 's/ensures r > x,/ensures r >= x,/' src/lib.rs
settle
pair bin-base
settle
pair bin-record
recorded bin-record
mkdir -p src/bin
printf 'use vstd::prelude::*;\n\nverus! {\nfn main() {}\n} // verus!\n' > src/bin/probe.rs
pair added-bin
expect added-bin '^miss: changed: '
grep -q 'demo v0.1.0' "$work/added-bin-sail.err"
rm -r src/bin
pair removed-bin
expect removed-bin '^miss: '
pair after-removed-bin
for label in removed-bin after-removed-bin; do
  if grep -qE '^ *(Compiling|Checking) ' "$work/$label-sail.err"; then
    echo "e2e: $label: Sail rebuilt after the binary was removed" >&2
    exit 1
  fi
done
rerecord after-bin
unset SAIL_MANAGED_NOOP_TRACE
echo "e2e: unchanged calls replay plain output; edits, restored mtimes, touches, files, binaries, declared variables, dep-info inputs, the lockfile, fingerprints, JSON and partial scope each miss ($((SECONDS - noop_start)) s)"
fi

if [ "$part" = flags ]; then
# Setups managed verification cannot bind to a proof receipt (ADR 0046)
# are verified as plain Cargo-Verus verifies them, with Cargo-Verus's own
# output and exit code and no Sail note (`SAIL_EXPLAIN=1` adds it). Each side
# runs once (both at once) to warm its target; the second runs are compared,
# with Cargo's `Finished ... in Ns` timing blanked.
unbound_case() { # label; rest = the arguments for both sides
  local label="$1"; shift
  touch src/lib.rs
  warm_plain "$label-plainwarm" stock_cargo verus verify "$@" &
  warm_sail "$label-sailwarm" "$sail" verify "$@" &
  wait
  plain_run "$label-plain" stock_cargo verus verify "$@"
  sail_run "$label-sail" "$sail" verify "$@"
  same "$label-plain" "$label-sail"
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
# An explicitly selected Cargo whose build-script protocol is unqualified
# keeps ordinary Cargo freshness. Only its reported source commit changes;
# compilation and all arguments still reach the real pinned Cargo.
unsupported_cargo_case() {
  local sail_output_real_cargo
  sail_output_real_cargo="$(command -v cargo)"
  mkdir "$work/unsupported-cargo"
  printf '%s\n' "$sail_output_real_cargo" > "$work/unsupported-cargo/real-cargo"
  cat > "$work/unsupported-cargo/cargo" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
real="$(cat "$(dirname "$0")/real-cargo")"
if [ "$#" = 2 ] && [ "$1" = --version ] && [ "$2" = --verbose ]; then
  "$real" "$@" | sed 's/^commit-hash: .*/commit-hash: 0000000000000000000000000000000000000000/'
else
  exec "$real" "$@"
fi
SH
  chmod +x "$work/unsupported-cargo/cargo"
  export CARGO="$work/unsupported-cargo/cargo"
  cat > build.rs <<'RS'
fn main() {
    println!("cargo:rerun-if-env-changed=CARGO_CFG_TARGET_OS");
    assert!(!std::env::var("CARGO_CFG_TARGET_OS").unwrap().is_empty());
}
RS
  plain_run unsupported-fail-plain stock_cargo verus verify
  sail_run unsupported-fail-sail "$sail" verify
  same unsupported-fail-plain unsupported-fail-sail
  [ "$(cat "$work/unsupported-fail-sail.rc")" = 101 ]
  cp src/lib.rs "$work/unsupported-failing.rs"
  edit_in_place 's/ensures r > x,/ensures r >= x,/' src/lib.rs
  plain_run unsupported-pass-plain stock_cargo verus verify
  sail_run unsupported-pass-sail "$sail" verify
  same unsupported-pass-plain unsupported-pass-sail
  [ "$(cat "$work/unsupported-pass-sail.rc")" = 0 ]
  CARGO_TARGET_DIR="$work/target-plain" quiet_run unsupported-noop-plain stock_cargo verus verify
  CARGO_TARGET_DIR="$work/target-sail" quiet_run unsupported-noop-sail "$sail" verify
  same unsupported-noop-plain unsupported-noop-sail
  if grep -qE '^ *(Compiling|Checking) ' "$work/unsupported-noop-sail.err"; then
    echo 'e2e: unsupported Cargo rebuilt an unchanged passing project' >&2
    exit 1
  fi
  CARGO_TARGET_DIR="$work/target-sail" "$sail" verify --output json > "$work/unsupported-json.out" 2> "$work/unsupported-json.err"
  python3 - "$work/unsupported-json.out" <<'PY'
import json,sys
result=json.load(open(sys.argv[1]))
assert result['currentness']=='unknown' and result['exit_code']==0, result
assert result['no_reuse']['code']=='SAIL3112', result['no_reuse']
assert result['no_reuse']['verified_as']=='plain-cargo-verus',result['no_reuse']
assert 'cannot track' in result['no_reuse']['reason'], result['no_reuse']
assert all(not t['reused'] and not t['complete'] for t in result['targets']),result['targets']
PY
  cp "$work/unsupported-failing.rs" src/lib.rs
  rm build.rs
  echo 'e2e: unsupported Cargo preserves failing and passing output, unchanged Cargo no-ops, and explicit no-reuse JSON'
}
( RUSTUP_TOOLCHAIN="$sail_stock_toolchain" unsupported_cargo_case )

# An explicit target directory: each side its own, already built by the
# cases above (a fresh one would add two cold builds to this job).
run target-dir-plain stock_cargo verus verify --target-dir "$work/target-plain"
run target-dir-sail "$sail" verify --target-dir "$work/target-sail"
same target-dir-plain target-dir-sail
# Last: plain Cargo-Verus then treats the crate as fresh for any later
# Verus arguments, since `--no-verify` builds it successfully.
unbound_case no-verify -- --no-verify
echo "e2e: --no-verify, --config, --tests and --target-dir verified as plain cargo verus verify"
fi

if [ "$part" = cargo-range ]; then
# Projects that pin their own Rust (ADR 0058 keeps the project's Cargo) with
# a build script that declares Cargo-generated inputs. Each qualified Cargo
# (ADR 0084) must match plain Cargo-Verus under the same toolchain file,
# establish proof receipts, reuse them, and replay unchanged calls. A real
# unqualified Cargo keeps the SAIL3139 fallback. Both sides select the
# toolchain only through the project's rust-toolchain.toml.
export SAIL_MANAGED_NOOP_TRACE="$work/noop.trace"
: > "$SAIL_MANAGED_NOOP_TRACE"
cp src/lib.rs "$work/range-failing.rs"
cat > build.rs <<'RS'
fn main() {
    for key in ["CARGO_CFG_TARGET_OS", "CARGO_CFG_DEBUG_ASSERTIONS", "CARGO_FEATURE_STD",
                "CARGO_PKG_VERSION", "CARGO_MANIFEST_DIR", "OUT_DIR"] {
        println!("cargo::rerun-if-env-changed={key}");
    }
    println!("cargo::rerun-if-changed=build.rs");
    assert!(!std::env::var("CARGO_CFG_TARGET_OS").unwrap().is_empty());
}
RS
range_json() { # label, then checks on the requested target
  CARGO_TARGET_DIR="$work/target-sail" quiet_run "$1" "$sail" verify --output json
  python3 - "$work/$1.out" "$2" "$1" <<'PY'
import json, sys
result, mode = json.load(open(sys.argv[1])), sys.argv[2]
[target] = [t for t in result["targets"] if t["requested"]]
summary = {k: result.get(k) for k in ("currentness", "exit_code", "no_reuse", "execution", "notes")}
summary["label"] = sys.argv[3]
if mode == "settle":
    assert result["exit_code"] == 0, summary
    sys.exit()
if mode == "unqualified":
    assert result["currentness"] == "unknown" and result["exit_code"] == 0, summary
    assert result["no_reuse"]["code"] == "SAIL3112", summary
    assert "cannot track" in result["no_reuse"]["reason"], summary
    assert not target["reused"] and not target["complete"], target
    sys.exit()
assert result["currentness"] == "current" and result["exit_code"] == 0, summary
assert result.get("no_reuse") is None, summary
assert target["complete"], target
if mode == "reused":
    assert target["reused"] and result["execution"]["verifier_invocations"] == 0, summary
PY
}
range_replay() { # label: record, then a replay that matches plain output
  local attempt
  for attempt in 1 2 3 4; do
    sleep 3
    CARGO_TARGET_DIR="$work/target-sail" quiet_run "$1-record" "$sail" verify
    if tail -n 1 "$SAIL_MANAGED_NOOP_TRACE" | grep -q '^recorded: '; then break; fi
  done
  tail -n 1 "$SAIL_MANAGED_NOOP_TRACE" | grep -q '^recorded: ' || {
    echo "e2e: $1: the unchanged call was never recorded" >&2; cat "$SAIL_MANAGED_NOOP_TRACE" >&2; exit 1; }
  CARGO_TARGET_DIR="$work/target-plain" quiet_run "$1-plain" cargo verus verify
  CARGO_TARGET_DIR="$work/target-sail" quiet_run "$1-sail" "$sail" verify
  same "$1-plain" "$1-sail"
  grep -E '^(hit|miss: )' "$SAIL_MANAGED_NOOP_TRACE" | tail -n 1 | grep -q '^hit$' || {
    echo "e2e: $1: the unchanged call was not replayed" >&2; cat "$SAIL_MANAGED_NOOP_TRACE" >&2; exit 1; }
}
range_case() { # toolchain qualified|unqualified
  local toolchain="$1" mode="$2" label="range-${1//./-}"
  printf '[toolchain]\nchannel = "%s"\n' "$toolchain" > rust-toolchain.toml
  if ! cargo --version | grep -q "^cargo $toolchain "; then
    echo "e2e: rust-toolchain.toml did not select Cargo $toolchain: $(cargo --version)" >&2
    exit 1
  fi
  cp "$work/range-failing.rs" src/lib.rs
  # Another Cargo rebuilds everything; cold output order is never compared.
  warm_plain "$label-warm-plain" cargo verus verify &
  warm_sail "$label-warm-sail" "$sail" verify &
  wait
  plain_run "$label-fail-plain" cargo verus verify
  sail_run "$label-fail-sail" "$sail" verify
  same "$label-fail-plain" "$label-fail-sail"
  [ "$(cat "$work/$label-fail-sail.rc")" = 101 ]
  edit_in_place 's/ensures r > x,/ensures r >= x,/' src/lib.rs
  plain_run "$label-pass-plain" cargo verus verify
  sail_run "$label-pass-sail" "$sail" verify
  same "$label-pass-plain" "$label-pass-sail"
  [ "$(cat "$work/$label-pass-sail.rc")" = 0 ]
  if [ "$mode" = unqualified ]; then
    range_json "$label-json" unqualified
    echo "e2e: unqualified Cargo $toolchain: plain output, SAIL3139 fallback without receipts"
    return
  fi
  # A newly scripted package can take one more call to converge (#399).
  range_json "$label-settle" settle
  range_json "$label-established" current
  # A receipt: unchanged, and after a touch Cargo rebuilds for, nothing is
  # verified again. A crate with a build script is outside instant revert's
  # profile with every Cargo (SAIL3146), so a reverted edit verifies again.
  range_json "$label-unchanged" reused
  touch src/lib.rs
  range_json "$label-touched" reused
  cp src/lib.rs "$work/range-passing.rs"
  printf '// edit\n' >> src/lib.rs
  range_json "$label-edited" current
  cp "$work/range-passing.rs" src/lib.rs
  range_json "$label-reverted" current
  grep -q SAIL3146 "$work/$label-reverted.out"
  range_replay "$label-replay"
  echo "e2e: Cargo $toolchain: plain output, receipts, reuse after a touch, edits verified, unchanged replay"
}
for toolchain in ${SAIL_CARGO_RANGE_E2E:-1.97.1 1.98.0 1.99.1}; do range_case "$toolchain" qualified; done
for toolchain in ${SAIL_CARGO_RANGE_E2E_UNQUALIFIED:-1.92.0}; do range_case "$toolchain" unqualified; done
rm rust-toolchain.toml build.rs
cp "$work/range-failing.rs" src/lib.rs
unset SAIL_MANAGED_NOOP_TRACE
fi

if [ "$part" = loader ]; then
# A process-loader variable: verified as plain Cargo-Verus verifies it, with
# the same output, no proof receipts and no dependency store; the JSON run and
# `SAIL_EXPLAIN=1` carry one note saying why. The failing proof
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
plain_run loader-plain stock_cargo verus verify
LD_LIBRARY_PATH="$work/no-libraries" CARGO_TARGET_DIR="$work/target-sail" \
  run loader-sail "$sail" verify
same loader-plain loader-sail
SAIL_EXPLAIN=1 LD_LIBRARY_PATH="$work/no-libraries" CARGO_TARGET_DIR="$work/target-sail" \
  run loader-explain "$sail" verify
[ "$(grep -c '^note\[SAIL3112\]' "$work/loader-explain.err")" = 1 ]
edit_in_place '/^note\[SAIL3112\]/d' "$work/loader-explain.err"
same loader-plain loader-explain
echo "e2e: LD_LIBRARY_PATH verified as plain cargo verus verify, without proof reuse"
fi

if [ "$part" = extra ]; then
# Verus's own stdout flags, as the conformance harness passes them. New
# Verus arguments are a new context for Sail (its first run verifies vstd
# again), so each side runs once first. The documents' timings and build
# information (Sail's Verus is built from the fork, ADR 0043) differ; their
# results must not.
CARGO_TARGET_DIR="$work/target-plain" edit_run json-plainwarm stock_cargo verus verify -- --output-json --time
CARGO_TARGET_DIR="$work/target-sail" edit_run json-sailwarm "$sail" verify -- --output-json --time
CARGO_TARGET_DIR="$work/target-plain" edit_run json-plain stock_cargo verus verify -- --output-json --time
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
  CARGO_TARGET_DIR="$work/target-ws-plain" ws_run ws-plainwarm stock_cargo verus verify -p b --features extra
  CARGO_TARGET_DIR="$work/target-ws-plain" ws_run ws-plain stock_cargo verus verify -p b --features extra
  ws_run ws-sailwarm "$sail" verify -p b --features extra
  ws_run ws-sail "$sail" verify -p b --features extra
  same ws-plain ws-sail
  grep -q "postcondition not satisfied" "$work/ws-plain.err"
)
echo "e2e: --output-json --time and a -p/--features workspace identical to cargo verus verify"
fi
