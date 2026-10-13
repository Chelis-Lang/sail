#!/usr/bin/env bash
# The CI steps. Every GitHub workflow runs these inside the CI image
# (ci/Dockerfile), and `ci/local.sh <step>` runs them in the same image
# locally, so CI and local results come from one definition.
#
#   gate                 fmt, clippy, tests, doctests, rustdoc, layering,
#                        cargo-deny, docs/benchmarks.md freshness
#   solver               the tests that need the pinned Z3 (real-solver,
#                        cached-session and fault-injection suites)
#   e2e [legacy|managed|repair] Run all phases by default; CI runs them in parallel.
#                        `sail verify` vs `cargo verus verify` on a fresh
#                        project, cold and warm: identical output and exit;
#                        then `sail verify --output json`: unchanged stderr
#                        and exit, a schema-v1 document agreeing with
#                        Verus's own --output-json; then one `verify` call
#                        through `sail mcp`, agreeing with that document;
#                        managed receipts, durable jobs and asynchronous MCP;
#                        repair CLI/MCP against a validated prebuilt engine
#   conformance TIER [pr | SELECTION...]
#                        plain Verus vs sail-z3 pass-through, then with the
#                        cache on (fill, then warm), then the source-engine
#                        arm (validated prebuilt engine vs plain Verus on
#                        verdicts and diagnostics). `pr` limits the fast
#                        tier to the guide examples; otherwise extra
#                        arguments (for example `--project anvil`) are
#                        passed to sail-conformance
#   host                 local-only, outside the image, on the developer's
#                        own machine (macOS or Linux): clippy and the
#                        non-solver tests
#   bench WORKLOAD TIER ROUNDS
#                        a sail-bench workload (cold|warm|edit)
#   build-verus-source [--force] [--keep-intermediates]
#                        a source build of the pinned Verus commit with its
#                        rust_verify_test binaries, under the cache
#   deep-rvt [cache] [ARGS...]
#                        deep tier: Verus's rust_verify_test suite, plain vs
#                        sail-z3 pass-through, per test (ADR 0019); with
#                        `cache`, then also with the cache daemon (fill,
#                        warm). ARGS go to `sail-conformance
#                        rust-verify-test` (for example --filter modes::)
#
# CI runs gate, solver, e2e and `conformance fast pr`. Every other tier
# and every benchmark is local-only: run them with ci/local.sh
# (`host` runs directly: ci/run.sh host).
set -euo pipefail
cd "$(dirname "$0")/.."

# CI runs only the fast, per-PR steps (ADR 0013). Long runs are local-only.
local_only() {
  if [ "${CI:-}" = true ]; then
    echo "ci/run.sh: '$*' is local-only (ADR 0013); run it locally (see AGENTS.md Gates)" >&2
    exit 2
  fi
}

step="${1:?usage: ci/run.sh gate|solver|e2e|conformance|host|bench|build-verus-source|deep-rvt ...}"
shift

release_build() {
  cargo build --release --locked -p sail-cli -p sail-shim "$@"
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
  sail_repair_source="${SAIL_CACHE_DIR:-$HOME/.cache/sail}/corpus/verus-426d8b01e7ff"
}

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
  gate)
    bash ci/test-completion.sh
    # The image's engine installer: corrupt or stale runtimes never install.
    python3 ci/test-engine-artifact.py
    python3 scripts/check-links.py
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
    cargo nextest run --workspace --locked --profile ci --run-ignored only
    ;;

  e2e)
    phase="${1:-full}"
    if [ "$#" -gt 1 ]; then
      echo 'e2e: expected at most one phase: legacy|managed|repair' >&2
      exit 2
    fi
    case "$phase" in
      full|legacy|managed|repair) ;;
      *) echo "e2e: unknown phase '$phase'; expected legacy|managed|repair" >&2; exit 2 ;;
    esac
    sail_checkout="$PWD"
    release_build
    install_runtime
    sail_managed_test="$sail_checkout/ci/test-managed.py"
    if [ "$phase" = full ] || [ "$phase" = legacy ]; then
      cargo build --release --locked -p sail-cli --example check_output --example check_mcp
      check_output="$PWD/target/release/examples/check_output"
      check_mcp="$PWD/target/release/examples/check_mcp"
      sail_portfolio_test="$PWD/ci/test-portfolio.sh"
      work="$(mktemp -d)"
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
      # Warm-up: the first build of a fresh project also verifies its vstd
      # dependency; afterwards every run below re-verifies only the crate.
      run warmup cargo verus verify
      run plain cargo verus verify
      run cold "$sail" verify
      run warm "$sail" verify
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
      # `--output json` (ADR 0014): stderr and exit code unchanged, stdout one
      # schema-v1 document whose verdicts agree with Verus's own --output-json.
      # The runs above filled the cache, so every answer is served from it.
      run json "$sail" verify --output json
      run verusjson cargo verus verify -- --output-json --time
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
      "$check_mcp" "$sail" "$PWD" "$work/json.out" "$work/mcp.json"
      "$check_output" "$work/mcp.json" "$work/verusjson.out" --attribution complete --source cache
      bash "$sail_portfolio_test"
    fi
    if [ "$phase" = full ] || [ "$phase" = managed ]; then
      sail_managed_evidence="${SAIL_MANAGED_ACCEPTANCE_OUT:-$sail_checkout/target/managed-acceptance}"
      # Bounded complete-report receipts: reuse, missing evidence and a failing
      # proof edit with restored timestamps, compared with the pinned Verus.
      # Keep timeout/error output outside the fixture's TemporaryDirectory.
      # Use the checkout path because the full legacy phase changes cwd.
      SAIL_MANAGED_ACCEPTANCE_OUT="$sail_managed_evidence" \
        python3 "$sail_managed_test" "$sail"
    fi
    if [ "$phase" = full ] || [ "$phase" = repair ]; then
      cd "$sail_checkout"
      : "${SAIL_ENGINE_BUILD:?repair acceptance requires the validated prebuilt engine}"
      release_build -p sail-conformance
      fetch_repair_source
      python3 "$PWD/ci/test-repair.py" --sail "$sail" --engine-build "$SAIL_ENGINE_BUILD" \
        --verus-source "$sail_repair_source" \
        --out "${SAIL_REPAIR_ACCEPTANCE_OUT:-$PWD/target/repair-acceptance}" \
        --environment-label "Linux x86-64 prebuilt-engine CI/local container"
    fi
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
    ;;

  bench)
    local_only bench "$@"
    workload="${1:?bench needs a workload: cold|warm|edit}"
    tier="${2:-fast}"
    rounds="${3:-3}"
    release_build -p sail-bench
    name="${workload}-${tier}-x86_64-linux-${SAIL_BENCH_LABEL:-ci}-$(date -u +%Y-%m-%d)"
    case "$workload" in
      cold|warm) ./target/release/sail-bench "$workload" --tier "$tier" --rounds "$rounds" --name "$name" ;;
      edit) ./target/release/sail-bench edit --rounds "$rounds" --name "$name" ;;
      *) echo "unknown workload $workload" >&2; exit 2 ;;
    esac
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
    cargo xtask build-verus-source
    release_build -p sail-conformance
    out="${SAIL_CONFORMANCE_OUT:-conformance-out}/rust-verify-test"
    # Keep local parallelism modest: 4 tests at a time unless --jobs says so.
    ./target/release/sail-conformance rust-verify-test --jobs 4 --out "$out/pass-through" "$@"
    if [ "$with_cache" = true ]; then
      start_daemon
      for round in fill warm; do
        SAIL_DAEMON_SOCKET="$sock" ./target/release/sail-conformance rust-verify-test --jobs 4 --out "$out/cache-$round" "$@"
      done
      echo "cache entries: $(find "$cache/v2" -type f | wc -l)"
    fi
    ;;

  *)
    echo "unknown step $step" >&2
    exit 2
    ;;
esac
