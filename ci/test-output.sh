#!/usr/bin/env bash
# The e2e `output` phase (ADR 0046): `sail verify`'s output against plain
# stock `cargo verus verify` under the toolchain Sail selects for its Cargo
# (`RUSTUP_TOOLCHAIN`, ADR 0058), byte for byte, across the edit loop and the
# flags that change Verus's or Cargo's output. Run by `ci/run.sh e2e output`
# inside the CI image, with Sail's Verus selected (SAIL_VERUS_BUILD) and
# stock `cargo verus` on PATH.
#
#   ci/test-output.sh SAIL loop|loader|flags|extra
#
# `loop`: combined output, passing proofs, an unchanged re-run, a compile
# error, partial verification and a seeded-mutation smoke. `loader`: a loader variable. `flags`: the
# setups verified without proof reuse. `extra`: `-- --output-json --time` and
# a `-p`/`--features` workspace. Every cold build verifies vstd (one to two
# minutes on a two-CPU CI runner), so CI runs `loop`, `loader` and `flags` as
# separate jobs with two each, run at once; `extra`, which needs six, is
# local-only (`ci/local.sh e2e extra`).
set -euo pipefail
sail="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
part="${2:?usage: ci/test-output.sh SAIL loop|loader|flags|extra}"
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
