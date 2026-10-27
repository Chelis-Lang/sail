#!/usr/bin/env bash
# The CI steps. Every GitHub workflow runs these inside the CI image
# (ci/Dockerfile), and `ci/local.sh <step>` runs them in the same image
# locally, so CI and local results come from one definition.
#
#   gate                 fmt, clippy, tests, doctests, rustdoc, layering,
#                        cargo-deny, docs/benchmarks.md freshness
#   proofs               direct pinned stock-Verus model proofs and all
#                        deliberate guard-removal controls; no Sail driver
#   refinements [query|inputs|authority [INDEX/COUNT]]
#                        source-shared bodies and all controls; default all roots
#   solver               the tests that need the pinned Z3 (real-solver,
#                        cached-session and fault-injection suites)
#   boundary-witness admission|shapes|phases
#                        local-only external compiler witnesses; requires
#                        frozen inputs and the owning bounded runner
#   cargo-range          local-only: the build-script protocol self-test
#                        (ADR 0084) against every qualified Cargo release
#                        (SAIL_CARGO_RANGE_TOOLCHAINS) and a divergent
#                        control (SAIL_CARGO_RANGE_DIVERGENT), installing
#                        them with rustup; SAIL_CARGO_RANGE_CANDIDATES only
#                        reports unqualified candidates' results
#   e2e-build DIR        CI only: build the e2e binaries once (release sail,
#                        sail-z3, sail-conformance, check_output, check_mcp)
#                        into DIR/e2e-bin.tar, which every e2e phase job
#                        unpacks (SAIL_E2E_PREBUILT) instead of building
#   e2e [legacy|managed|managed-copies|managed-native|contracts|scripts|repair|output|loader|flags|extra|cargo-range] Run all phases by default; CI runs them in parallel.
#                        `sail new` writes what stock `cargo verus new`
#                        writes, and its lib and bin projects verify;
#                        `sail verify` (managed verification, the default,
#                        ADR 0046, on Sail's Verus, the fork runtime,
#                        ADR 0043) vs stock `cargo verus verify` (with the
#                        toolchain Sail selects for its Cargo, ADR 0058) on
#                        a fresh project, cold and warm: identical output
#                        and exit;
#                        then `sail verify --output json`: unchanged stderr
#                        and exit, a sail-project-result whose requested
#                        report agrees with Verus's own --output-json; then
#                        one `verify` call through `sail mcp`, agreeing with
#                        that document; managed receipts, durable jobs and
#                        asynchronous MCP (`managed`), and reuse across build
#                        scripts (`scripts`; one project with `managed` when
#                        all phases run); repair CLI/MCP against a
#                        validated prebuilt engine
#   conformance TIER [pr | SELECTION...]
#                        plain stock Verus vs Sail's Verus (the fork) run as
#                        `sail verify` runs it (sail-z3 and the managed
#                        driver's logging flags; ADR 0046): without the
#                        daemon, then with the cache on (fill, then warm),
#                        then the source-engine arm (validated prebuilt
#                        engine vs plain Verus on verdicts and diagnostics).
#                        standard and deep (without a selection) then run
#                        the mutation suite (ADR 0063): two mutants per
#                        class of human-eval, or every catalogue;
#                        `pr` limits the fast tier to the guide examples;
#                        otherwise extra arguments (for example
#                        `--project anvil`) are passed to sail-conformance
#   extension            the temporal extension workspace (extensions/temporal,
#                        ADR 0083): fmt, clippy and tests. The only step
#                        that needs the private D.I.V.E. sources, so the
#                        only one given SAIL_DEPENDENCY_READ_TOKEN
#   host                 local-only, outside the image, on the developer's
#                        own machine (macOS or Linux): clippy and the
#                        non-solver tests
#   bench WORKLOAD TIER ROUNDS
#                        a sail-bench workload (cold|warm|edit); `bench
#                        mutants NAME ROUNDS` runs corpus/mutants/NAME.toml
#   release              local-only: stage the Linux release tree (binaries
#                        and the prebuilt engine) in $SAIL_RELEASE_OUT for
#                        scripts/release.sh; SAIL_RELEASE_TEMPORAL=1 stages
#                        the internal temporal-enabled variant instead
#   build-verus-source [--force] [--keep-intermediates] [--fork]
#                        a source build of the pinned Verus commit with its
#                        rust_verify_test binaries, under the cache
#   deep-rvt [cache] [ARGS...]
#                        deep tier: the fork's rust_verify_test suite with
#                        exact fork vs sail-z3 checks, then the original
#                        stock-with-and-without-shim control (ADR 0042);
#                        with
#                        `cache`, then also with the cache daemon (fill,
#                        warm). ARGS go to `sail-conformance
#                        rust-verify-test` (for example --filter modes::)
#
# Draft policy pending owner resolution: retain the required proof jobs.
# CI runs gate, proofs, refinements, solver, e2e and `conformance fast pr`.
# Every other tier (including `extension`)
# and every benchmark is local-only: run them with ci/local.sh
# (`host` runs directly: ci/run.sh host).
set -euo pipefail
# Only the extension workspace's command-time Git access receives the
# read-only private-dependency secret (ADR 0083). It is never a Docker build
# argument or a persisted Git URL/configuration, and every other step builds
# the main workspace, which resolves no private source.
case "${1:-}" in
  extension|release)
    if [ -n "${SAIL_DEPENDENCY_READ_TOKEN:-}" ] && [ "${SAIL_PRIVATE_GIT_READY:-}" != 1 ]; then
      exec "$(dirname "$0")/../scripts/with-private-dependencies.sh" "$0" "$@"
    fi
    ;;
esac

cd "$(dirname "$0")/.."

# CI runs only the fast, per-PR steps (ADR 0013). Long runs are local-only.
local_only() {
  if [ "${CI:-}" = true ]; then
    echo "ci/run.sh: '$*' is local-only (ADR 0013); run it locally (see AGENTS.md Gates)" >&2
    exit 2
  fi
}

step="${1:?usage: ci/run.sh gate|solver|boundary-witness|cargo-range|e2e-build|e2e|conformance|extension|host|bench|release|build-verus-source|deep-rvt ...}"
shift

release_build() {
  cargo build --release --locked -p sail-cli -p sail-shim "$@"
}

# The temporal extension workspace, built into its own directory under
# target/ (one cache and container volume for both workspaces).
extension_cargo=(--manifest-path extensions/temporal/Cargo.toml --target-dir target/temporal)

# The binaries every e2e phase uses, relative to target/release.
e2e_binaries=(sail sail-z3 sail-conformance examples/check_output examples/check_mcp examples/check_contract_jobs)

e2e_build() {
  release_build -p sail-conformance --bins --example check_output --example check_mcp --example check_contract_jobs
}

# Unpacks the binaries `e2e-build` packed in this CI run, checking every
# file against the digests packed with them; a missing or altered file
# fails the phase.
unpack_e2e_binaries() {
  mkdir -p target/release
  tar -xf "$1" -C target/release
  (cd target/release && sha256sum --quiet -c e2e-bin.sha256)
}

# Copies the release runtime under /tmp and sets $sail. A host-mounted
# checkout can have an unsupported FUSE ancestor even when target/ is a Linux
# volume. Execute exact copied runtime bytes under /tmp, so managed freshness
# can observe every runtime path on the native kernel.
install_runtime() {
  sail_runtime="$(mktemp -d /tmp/sail-e2e-runtime.XXXXXX)"
  trap 'rm -rf "$sail_runtime"' EXIT
  cp "$sail_checkout/target/release/sail" "$sail_checkout/target/release/sail-z3" "$sail_runtime/"
  chmod 700 "$sail_runtime/sail" "$sail_runtime/sail-z3"
  sail="$sail_runtime/sail"
}

# Fetches the pinned Verus source that repair fixtures use for Cargo's
# ordinary builtin/macro dependencies; sets $sail_repair_source. The engine
# runtime comes from the image; this never builds Verus.
fetch_repair_source() {
  ./target/release/sail-conformance fetch --tier fast --project verus-examples
  sail_repair_source="${SAIL_CACHE_DIR:-$HOME/.cache/sail}/corpus/verus-7d829946231d"
}

# Every qualified Cargo release (QUALIFIED_CARGO in
# crates/sail-verus/src/managed/build_environment.rs, ADR 0084).
sail_cargo_range_qualified="1.93.0 1.93.1 1.94.0 1.94.1 1.95.0 1.96.0 1.96.1 1.97.0 1.97.1 1.98.0 1.98.1 1.99.0 1.99.1"

# Starts a cache daemon on a fresh cache; sets $sock, $cache and $daemon.
start_daemon() {
  sock="$HOME/.cache/sail/ci-$$.sock"
  cache="$(mktemp -d)/cache"
  mkdir -p "$(dirname "$sock")"
  ./target/release/sail serve --socket "$sock" --cache "$cache" --idle-timeout 0 &
  daemon=$!
  trap 'kill "$daemon" 2>/dev/null || true' EXIT
  bash "$PWD/ci/wait-daemon.sh" "$sock" "$daemon"
}

case "$step" in
  proofs|refinements)
    sail_proof_script=scripts/check-acceptance-proofs.py
    sail_proof_artifact=acceptance-proofs
    if [ "$step" = refinements ]; then
      sail_proof_script=scripts/check-acceptance-refinements.py
      sail_proof_artifact=acceptance-refinements
    fi
    sail_proof_options=()
    if [ "$#" -gt 0 ]; then
      if [ "$step" != refinements ] || [ "$#" -gt 2 ]; then
        echo 'proofs accepts no selector; refinements accepts COMPONENT [INDEX/COUNT]' >&2
        exit 1
      fi
      case "$1" in
        query|inputs|authority) sail_proof_options=(--component "$1");;
        *) echo 'unknown refinement component' >&2; exit 1;;
      esac
      if [ "$#" -eq 2 ]; then
        sail_proof_options+=(--partition "$2")
      fi
    fi
    rm -rf "target/$sail_proof_artifact"
    mkdir -p "target/$sail_proof_artifact"
    sail_proof_tmp="$(mktemp -d)"
    # Git trust is restricted to this already executing checkout. No private
    # dependency authentication or injected Git helper is needed for blob reads.
    if env GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0=safe.directory GIT_CONFIG_VALUE_0="$PWD" \
      python3 "$sail_proof_script" \
        --stock-archive /opt/verus/verus.zip \
        --stock-dir /opt/verus/verus-x86-linux \
        --out "$sail_proof_tmp/checked" ${sail_proof_options[@]+"${sail_proof_options[@]}"}; then
      sail_proof_status=0
    else
      sail_proof_status=$?
    fi
    if [ -d "$sail_proof_tmp/checked" ]; then
      cp -R "$sail_proof_tmp/checked/." "target/$sail_proof_artifact/"
    fi
    rm -rf "$sail_proof_tmp"
    exit "$sail_proof_status"
    ;;
  gate)
    bash ci/test-completion.sh
    bash scripts/test-ai-markers.sh
    # The image's engine installer: corrupt or stale runtimes never install.
    python3 ci/test-engine-artifact.py
    python3 ci/test-output-evidence.py
    python3 ci/test-timeout.py
    python3 scripts/check-links.py
    python3 scripts/test-perf-matrix.py
    python3 scripts/test-private-dependencies.py
    python3 scripts/test-check-acceptance-proofs.py
    python3 scripts/test-check-acceptance-refinements.py
    python3 scripts/test-install.py
    python3 scripts/test-stage-temporal-worker.py
    python3 scripts/test-bench-cargo-compare.py
    cargo fmt --all --check
    cargo clippy --workspace --all-targets --locked -- -D warnings
    cargo nextest run --workspace --locked --profile ci
    # check_mcp drives the e2e legacy phase; its own unit tests stay here.
    cargo test --locked -p sail-cli --example check_mcp
    cargo test --workspace --doc --locked
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked
    cargo xtask check-layering
    cargo deny --locked check
    mkdir -p target
    cargo run -q --locked -p sail-bench -- report --out target/benchmarks.md bench/results/*.json
    diff -u docs/benchmarks.md target/benchmarks.md
    ;;

  solver)
    : "${SAIL_TEST_Z3:?SAIL_TEST_Z3 must name the pinned z3 (set in the CI image)}"
    # Compiler-boundary witnesses require separately captured actual inputs;
    # cargo-range needs every qualified/divergent toolchain. Keep all four
    # strict local-only entrypoints separate from pinned-Z3 solver fixtures.
    cargo nextest run --workspace --locked --profile ci --run-ignored only \
      -E 'not (test(=managed::shipped::native::admission_actual_tests::actual_native_admission_child) or test(=managed::shipped::native::native_shape::tests::actual_retained_native_shapes) or test(=managed::retained::phase_fields::tests::actual_compiler_phase_record_preserves_ghost_environment) or test(=managed::build_environment::tests::qualified_cargo_range_self_test))'
    ;;

  cargo-range)
    # The build-script protocol self-test (ADR 0084) against real rustup
    # toolchains: every qualified Cargo must pass; every divergent one must
    # be unqualified and fail. Installs the toolchains (network), so local-only.
    local_only cargo-range
    export SAIL_CARGO_RANGE_TOOLCHAINS="${SAIL_CARGO_RANGE_TOOLCHAINS:-$sail_cargo_range_qualified}"
    export SAIL_CARGO_RANGE_DIVERGENT="${SAIL_CARGO_RANGE_DIVERGENT:-1.87.0}"
    for sail_toolchain in $SAIL_CARGO_RANGE_TOOLCHAINS $SAIL_CARGO_RANGE_DIVERGENT ${SAIL_CARGO_RANGE_CANDIDATES:-}; do
      rustup toolchain install "$sail_toolchain" --profile minimal
    done
    cargo nextest run -p sail-verus --lib --locked --profile ci --run-ignored only --no-capture \
      -E 'test(=managed::build_environment::tests::qualified_cargo_range_self_test)'
    ;;

  boundary-witness)
    local_only boundary-witness
    if [ "$#" -ne 1 ]; then
      echo 'boundary-witness: expected admission|shapes|phases' >&2
      exit 2
    fi
    case "$1" in
      admission)
        : "${SAIL_ACTUAL_ADMISSION_CONFIG:?requires frozen actual compiler admission inputs}"
        sail_witness_test='managed::shipped::native::admission_actual_tests::actual_native_admission_child'
        ;;
      shapes)
        : "${SAIL_ACTUAL_NATIVE_TUPLES:?requires retained genuine Cargo tuple evidence}"
        sail_witness_test='managed::shipped::native::native_shape::tests::actual_retained_native_shapes'
        ;;
      phases)
        : "${SAIL_ACTUAL_PHASE_BUILD:?requires the frozen pinned native fork build}"
        : "${SAIL_ACTUAL_PHASE_OUT:?requires a fresh external output directory}"
        sail_witness_test='managed::retained::phase_fields::tests::actual_compiler_phase_record_preserves_ghost_environment'
        ;;
      *) echo 'boundary-witness: expected admission|shapes|phases' >&2; exit 2 ;;
    esac
    cargo nextest run -p sail-verus --lib --locked --profile ci --run-ignored only \
      -E "test(=$sail_witness_test)"
    ;;

  e2e-build)
    mkdir -p "${1:?usage: ci/run.sh e2e-build DIR}"
    out="$(cd "$1" && pwd)"
    e2e_build
    (cd target/release && sha256sum "${e2e_binaries[@]}" > e2e-bin.sha256 \
      && tar -cf "$out/e2e-bin.tar" "${e2e_binaries[@]}" e2e-bin.sha256)
    ;;

  e2e)
    phase="${1:-full}"
    if [ "$#" -gt 1 ]; then
      echo 'e2e: expected at most one phase: legacy|managed|managed-copies|managed-native|contracts|scripts|repair|output|loader|flags|extra|cargo-range' >&2
      exit 2
    fi
    case "$phase" in
      full|legacy|managed|managed-copies|managed-native|contracts|scripts|repair|output|loader|flags|extra|cargo-range) ;;
      *) echo "e2e: unknown phase '$phase'; expected legacy|managed|managed-copies|managed-native|contracts|scripts|repair|output|loader|flags|extra|cargo-range" >&2; exit 2 ;;
    esac
    sail_checkout="$PWD"
    sail_output_evidence="${SAIL_E2E_OUTPUT_OUT:-$sail_checkout/target/e2e-output-evidence}"
    unset SAIL_E2E_OUTPUT_OUT
    # Sail runs its own Verus, the validated fork runtime the image installs
    # (ADR 0043); stock `cargo verus` on PATH is only the oracle.
    : "${SAIL_ENGINE_BUILD:?e2e needs the validated prebuilt fork runtime, the Verus Sail runs}"
    export SAIL_VERUS_BUILD="$SAIL_ENGINE_BUILD"
    # CI builds once for every phase (`e2e-build`); locally, build here.
    if [ -n "${SAIL_E2E_PREBUILT:-}" ]; then
      unpack_e2e_binaries "$SAIL_E2E_PREBUILT"
    else
      e2e_build
    fi
    install_runtime
    sail_managed_test="$sail_checkout/ci/test-managed.py"
    if [ "$phase" = full ] || [ "$phase" = contracts ]; then
      contract_out="${SAIL_CONTRACT_JOBS_OUT:-$(mktemp -d /tmp/sail-contract-jobs.XXXXXX)/evidence}"
      echo "neutral service evidence: $contract_out"
      SAIL_VERUS_BUILD="${SAIL_ENGINE_BUILD:?neutral service requires the pinned engine}" \
        env -u LD_LIBRARY_PATH -u DYLD_LIBRARY_PATH -u DYLD_FRAMEWORK_PATH \
        "$sail_checkout/target/release/examples/check_contract_jobs" "$sail" "$contract_out"
    fi
    if [ "$phase" = full ] || [ "$phase" = legacy ]; then
      check_output="$PWD/target/release/examples/check_output"
      check_mcp="$PWD/target/release/examples/check_mcp"
      sail_portfolio_test="$PWD/ci/test-portfolio.sh"
      work="$(mktemp -d)"
      export SAIL_CACHE_DIR="$work/cache"
      # Stock runs with the toolchain Sail selects for its Cargo (ADR 0058).
      sail_stock_toolchain="$(python3 "$sail_checkout/ci/stock-toolchain.py" "$sail")"
      stock_cargo() { RUSTUP_TOOLCHAIN="$sail_stock_toolchain" cargo "$@"; }
      cd "$work"
      cargo verus new --lib demo >/dev/null
      # `sail new` (#148): the project stock `cargo verus new` at the pinned
      # release writes, so its vstd is the pinned one, and it verifies as
      # created. The first run also builds vstd into the Sail target dir.
      for kind in lib bin; do
        mkdir "$work/sail-new-$kind"
        (cd "$work/sail-new-$kind" && "$sail" new "--$kind" demo)
      done
      if ! diff -r -x .git "$work/demo" "$work/sail-new-lib/demo"; then
        echo "e2e: sail new --lib differs from stock cargo verus new --lib" >&2
        exit 1
      fi
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
      # Stock (the oracle) keeps its own Cargo target directory. Managed
      # `sail verify` builds in its own target under the Sail cache whatever
      # CARGO_TARGET_DIR says; the variable reaches only setups it verifies
      # as plain Cargo-Verus does (ADR 0046), which then do not share stock's.
      plain_run() { CARGO_TARGET_DIR="$work/target-plain" run "$@"; }
      sail_run() { CARGO_TARGET_DIR="$work/target-sail" run "$@"; }
      # Warm-up: the first build of a fresh project also builds its vstd
      # dependency; afterwards every run below re-verifies only the crate.
      # Stock's cold build runs while the `sail new` projects verify (their
      # first run verifies vstd for Sail); neither's output is compared.
      plain_run warmup stock_cargo verus verify &
      plain_warmup=$!
      cd "$work"
      for kind in lib bin; do
        (cd "$work/sail-new-$kind/demo" && CARGO_TARGET_DIR="$work/target-sail" "$sail" verify) \
          > "$work/new-$kind.out" 2> "$work/new-$kind.err" || {
          echo "e2e: the project sail new --$kind created does not verify" >&2
          cat "$work/new-$kind.out" "$work/new-$kind.err" >&2
          exit 1
        }
        grep -q "verification results:: 1 verified, 0 errors" "$work/new-$kind.out" "$work/new-$kind.err"
      done
      echo "e2e: sail new: lib identical to cargo verus new; lib and bin verify"
      wait "$plain_warmup"
      cd demo
      plain_run plain stock_cargo verus verify
      sail_run sailwarmup "$sail" verify
      sail_run cold "$sail" verify
      sail_run warm "$sail" verify
      for label in cold warm; do
        for part in rc out err; do
          if ! cmp -s "$work/plain.$part" "$work/$label.$part"; then
            echo "e2e: $label $part differs from plain cargo verus verify" >&2
            diff -u "$work/plain.$part" "$work/$label.$part" >&2 || true
            exit 1
          fi
        done
      done
      # The deliberate failure must be reported, and the cache must be used.
      grep -q "postcondition not satisfied" "$work/plain.err"
      entries="$(find "$SAIL_CACHE_DIR/results/v2" -type f | wc -l)"
      echo "e2e: identical to cargo verus verify (exit $(cat "$work/plain.rc")); cache entries: $entries"
      [ "$entries" -gt 0 ]
      # `--output json` (ADR 0014, ADR 0046): stderr and exit code unchanged,
      # stdout one sail-project-result whose requested target's schema-v2
      # report agrees with Verus's own --output-json. The runs above filled
      # the cache, so every answer is served from it.
      sail_run json "$sail" verify --output json
      plain_run verusjson stock_cargo verus verify -- --output-json --time
      for part in rc err; do
        if ! cmp -s "$work/plain.$part" "$work/json.$part"; then
          echo "e2e: --output json $part differs from plain cargo verus verify" >&2
          diff -u "$work/plain.$part" "$work/json.$part" >&2 || true
          exit 1
        fi
      done
      "$check_output" "$work/json.out" "$work/verusjson.out" --attribution complete --source cache
      # `sail mcp` (ADR 0016): one real `verify` tool call. Its verdicts must
      # equal `sail verify --output json`'s, and its report must pass the same
      # checks against Verus's own --output-json.
      touch src/lib.rs
      CARGO_TARGET_DIR="$work/target-sail" "$check_mcp" "$sail" "$PWD" "$work/json.out" "$work/mcp.json"
      "$check_output" "$work/mcp.json" "$work/verusjson.out" --attribution complete --source cache
      bash "$sail_portfolio_test"
    fi
    if [ "$phase" = full ] || [ "$phase" = output ]; then
      # `sail verify` against plain `cargo verus verify` across the edit loop
      # (ADR 0046), then unchanged-call replay and its misses (ADR 0082).
      (cd "$sail_checkout" && SAIL_E2E_OUTPUT_OUT="$sail_output_evidence" bash ci/test-output.sh "$sail" loop noop)
    fi
    if [ "$phase" = full ] || [ "$phase" = loader ]; then
      # A loader variable: verified without proof reuse, with one note.
      (cd "$sail_checkout" && SAIL_E2E_OUTPUT_OUT="$sail_output_evidence" bash ci/test-output.sh "$sail" loader)
    fi
    if [ "$phase" = full ] || [ "$phase" = flags ]; then
      # The setups verified as plain Cargo-Verus verifies them.
      (cd "$sail_checkout" && SAIL_E2E_OUTPUT_OUT="$sail_output_evidence" bash ci/test-output.sh "$sail" flags)
    fi
    if [ "$phase" = extra ] || { [ "$phase" = full ] && [ "${CI:-}" != true ]; }; then
      # `-- --output-json --time` and a workspace: four cold builds, so
      # local-only (ADR 0013).
      local_only e2e extra
      (cd "$sail_checkout" && SAIL_E2E_OUTPUT_OUT="$sail_output_evidence" bash ci/test-output.sh "$sail" extra)
    fi
    if [ "$phase" = cargo-range ]; then
      # Projects pinning another qualified Cargo, and an unqualified one
      # (ADR 0084): several cold builds per toolchain, so local-only and
      # never part of `full`.
      local_only e2e cargo-range
      for sail_toolchain in ${SAIL_CARGO_RANGE_E2E:-1.97.1 1.98.0 1.99.1} ${SAIL_CARGO_RANGE_E2E_UNQUALIFIED:-1.92.0}; do
        rustup toolchain install "$sail_toolchain" --profile minimal
      done
      (cd "$sail_checkout" && SAIL_E2E_OUTPUT_OUT="$sail_output_evidence" bash ci/test-output.sh "$sail" cargo-range)
    fi
    if [ "$phase" = full ] || [ "$phase" = managed ] || [ "$phase" = managed-copies ] || [ "$phase" = managed-native ] || [ "$phase" = scripts ]; then
      sail_managed_evidence="${SAIL_MANAGED_ACCEPTANCE_OUT:-$sail_checkout/target/managed-acceptance}"
      # Bounded complete-report receipts: reuse, missing evidence and a failing
      # proof edit with restored timestamps, compared with the pinned Verus.
      # Fresh copies, native compilation and build-script cases run in their
      # own CI phases; the full local run checks every case on one project.
      # Keep timeout/error output outside the fixture's TemporaryDirectory.
      # Use the checkout path because the full legacy phase changes cwd.
      case "$phase" in
        full) managed_part=all ;;
        managed) managed_part=core ;;
        managed-copies) managed_part=copies ;;
        managed-native) managed_part=native ;;
        scripts) managed_part=scripts ;;
      esac
      SAIL_MANAGED_ACCEPTANCE_OUT="$sail_managed_evidence" \
        python3 "$sail_managed_test" "$sail" "$managed_part"
      if [ "$phase" = full ] || [ "$phase" = managed ]; then
        # A real MCP client (pinned Claude Code) must list sail's tools.
        bash "$sail_checkout/ci/test-claude-code.sh" "$sail"
      fi
    fi
    if [ "$phase" = full ] || [ "$phase" = repair ]; then
      cd "$sail_checkout"
      : "${SAIL_ENGINE_BUILD:?repair acceptance requires the validated prebuilt engine}"
      fetch_repair_source
      python3 "$PWD/ci/test-repair.py" --sail "$sail" --engine-build "$SAIL_ENGINE_BUILD" \
        --verus-source "$sail_repair_source" \
        --out "${SAIL_REPAIR_ACCEPTANCE_OUT:-$PWD/target/repair-acceptance}" \
        --environment-label "Linux x86-64 prebuilt-engine CI/local container"
    fi
    ;;

  extension)
    # The temporal extension workspace (ADR 0083): its own lock, the pinned
    # private D.I.V.E. sources, and Sail's crates by path. Its tests cover the
    # native worker, the D.I.V.E. client and backend, the shared runtime
    # endpoint with the backend linked, and the temporal-enabled `sail`.
    cargo fmt --all --check --manifest-path extensions/temporal/Cargo.toml
    cargo clippy "${extension_cargo[@]}" --workspace --all-targets --locked -- -D warnings
    cargo nextest run "${extension_cargo[@]}" --workspace --locked \
      --config-file .config/nextest.toml --profile ci
    ;;

  host)
    # Code must build, lint and test on macOS and Linux alike. CI runs only
    # Linux, so run this on a Mac before pushing platform-specific changes.
    local_only host "$@"
    cargo clippy --workspace --all-targets --locked -- -D warnings
    cargo nextest run --workspace --locked --profile ci
    ;;

  conformance)
    tier="${1:-fast}"
    shift || true
    [ "$tier" = fast ] && [ "${1:-}" = pr ] || local_only conformance "$tier" "$@"
    release_build -p sail-conformance
    select=(--tier "$tier" "$@")
    if [ "${1:-}" = pr ]; then
      select=(--tier fast --project verus-examples --filter /guide/)
    fi
    if [ "$tier" = fast ]; then jobs=4; else jobs=3; fi
    out="${SAIL_CONFORMANCE_OUT:-conformance-out}"
    # The Sail side runs Verus as `sail verify` does (ADR 0046): first with
    # no daemon, as when the cache is unavailable.
    ./target/release/sail-conformance run "${select[@]}" --jobs "$jobs" --out "$out/pass-through"
    # With the cache on: fill, then warm. A difference in either is a failure.
    start_daemon
    for round in fill warm; do
      SAIL_DAEMON_SOCKET="$sock" ./target/release/sail-conformance run "${select[@]}" --jobs "$jobs" --out "$out/cache-$round"
    done
    entries="$(find "$cache/v2" -type f | wc -l)"
    echo "cache entries: $entries"
    [ "$entries" -gt 0 ]
    # Source-engine arm (ADR 0027): the validated prebuilt engine's focused
    # and full phases, the full phase compared with plain Verus like the arms
    # above. The CI image always provides the engine; a host run without one
    # must opt out explicitly, and CI cannot.
    if [ "${SAIL_CONFORMANCE_ENGINE:-1}" = 0 ] && [ "${CI:-}" != true ]; then
      echo "conformance: engine arm explicitly skipped (SAIL_CONFORMANCE_ENGINE=0)" >&2
    else
      : "${SAIL_ENGINE_BUILD:?the engine arm needs the validated prebuilt engine (SAIL_ENGINE_BUILD)}"
      ./target/release/sail-conformance engine "${select[@]}" --jobs "$jobs" \
        --engine-build "$SAIL_ENGINE_BUILD" --out "$out/engine"
    fi
    # Mutation suite (ADR 0063), one round, as a soundness probe: standard
    # runs two mutants per class of human-eval, deep every catalogue. Any
    # difference, false verified or failed revert check fails the step.
    # Skipped with a project selection, which names corpus projects only.
    if [ "$tier" != fast ] && [ "$#" -eq 0 ]; then
      cargo build --release --locked -p sail-bench
      if [ "$tier" = standard ]; then
        ./target/release/sail-bench mutants run --catalogue corpus/mutants/human-eval.toml \
          --per-class 2 --rounds 1 --vstd-lock scripts/shipped-deps/Cargo.lock --out "$out/mutants/human-eval"
      else
        for catalogue in corpus/mutants/*.toml; do
          name="$(basename "$catalogue" .toml)"
          ./target/release/sail-bench mutants run --catalogue "$catalogue" \
            --rounds 1 --vstd-lock scripts/shipped-deps/Cargo.lock --out "$out/mutants/$name"
        done
      fi
    fi
    ;;

  bench)
    local_only bench "$@"
    workload="${1:?bench needs a workload: cold|warm|edit|mutants}"
    tier="${2:-fast}"
    rounds="${3:-3}"
    release_build -p sail-bench
    name="${workload}-${tier}-x86_64-linux-${SAIL_BENCH_LABEL:-ci}-$(date -u +%Y-%m-%d)"
    case "$workload" in
      cold|warm) ./target/release/sail-bench "$workload" --tier "$tier" --rounds "$rounds" --name "$name" ;;
      edit) ./target/release/sail-bench edit --rounds "$rounds" --name "$name" ;;
      # `bench mutants NAME ROUNDS`: corpus/mutants/NAME.toml (ADR 0063);
      # timings only on a native host.
      mutants) ./target/release/sail-bench mutants run --catalogue "corpus/mutants/$tier.toml" --vstd-lock scripts/shipped-deps/Cargo.lock \
        --rounds "$rounds" --out "${SAIL_BENCH_OUT:-$HOME/sail-mutants}/$tier-$(date -u +%Y%m%dT%H%M%S)" ;;
      *) echo "unknown workload $workload" >&2; exit 2 ;;
    esac
    ;;

  release)
    # The Linux release tree (scripts/release.sh): release binaries and the
    # image's validated prebuilt engine, staged in $SAIL_RELEASE_OUT.
    local_only release "$@"
    release_build
    if [ "${SAIL_RELEASE_TEMPORAL:-}" = 1 ]; then
      # Internal temporal-enabled variant (ADR 0083); never a public archive.
      cargo build "${extension_cargo[@]}" --release --locked -p sail-temporal -p sail-runtime-worker
    fi
    # The disposable CI container also installs verismo's bare-metal target.
    # A distributed proof uses the README's minimal verifier toolchain; its
    # exact sysroot identity includes every installed target library.
    # Keep the shared image and other CI steps intact.
    if [ "${RUSTUP_HOME:-}" = /opt/rustup ] && [ -f /.dockerenv ]; then
      sail_release_tc="$(python3 -c 'import json, os, pathlib; print(json.loads((pathlib.Path(os.environ["SAIL_ENGINE_BUILD"]) / "sail-build.json").read_text())["rust_toolchain"])')"
      RUSTUP_PERMIT_COPY_RENAME=1 rustup target remove --toolchain "$sail_release_tc" x86_64-unknown-none
    fi
    bash scripts/release.sh stage "${SAIL_RELEASE_OUT:?release needs SAIL_RELEASE_OUT}" "${SAIL_ENGINE_BUILD:?release needs the validated prebuilt engine}"
    ;;

  build-verus-source)
    local_only build-verus-source "$@"
    cargo xtask build-verus-source "$@"
    ;;

  singular)
    local_only singular "$@"
    cargo xtask build-verus-source --features=singular
    source_build="$(cargo xtask build-verus-source --features=singular --print-path)"
    export SAIL_VERUS_SOURCE_BUILD="$source_build"
    bash ci/run.sh conformance deep --project verus-integer-ring --project verified-memory-allocator "$@"
    ;;

  deep-rvt)
    local_only deep-rvt "$@"
    with_cache=false
    if [ "${1:-}" = cache ]; then with_cache=true; shift; fi
    cargo xtask build-verus-source --fork
    cargo xtask build-verus-source
    release_build -p sail-conformance
    out="${SAIL_CONFORMANCE_OUT:-conformance-out}/rust-verify-test"
    # Keep local parallelism modest: 4 tests at a time unless --jobs says so.
    ./target/release/sail-conformance rust-verify-test --jobs 4 --out "$out/pass-through" "$@" --baseline fork
    ./target/release/sail-conformance rust-verify-test --jobs 4 --out "$out/stock-oracle" "$@" --baseline stock
    if [ "$with_cache" = true ]; then
      start_daemon
      for round in fill warm; do
        SAIL_DAEMON_SOCKET="$sock" ./target/release/sail-conformance rust-verify-test --jobs 4 --out "$out/cache-$round" "$@" --baseline fork
      done
      echo "cache entries: $(find "$cache/v2" -type f | wc -l)"
    fi
    ;;

  *)
    echo "unknown step $step" >&2
    exit 2
    ;;
esac
