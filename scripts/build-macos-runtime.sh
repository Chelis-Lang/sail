#!/usr/bin/env bash
# Builds, qualifies and packs the macOS (Apple silicon) engine runtime for the
# fork commit in engine/verus/fork.toml (engine/verus/README.md, "Release
# archives"; docs/validation/macos-engine.md). Local only; never publishes.
#
#   scripts/build-macos-runtime.sh build [WORK]   build, qualify, pack
#   scripts/build-macos-runtime.sh published      after publishing: fetch the
#                                                 pinned runtime anonymously and
#                                                 check it (conformance, VI-0018)
#
# `build` runs, in this order, and stops at the first failure:
#   1. `cargo xtask build-verus-engine` with RUSTUP_HOME, CARGO_HOME and
#      SAIL_CACHE_DIR under WORK (default /tmp/sail-engine-<fork short>), so no
#      home-directory path is compiled into the runtime (panic locations and
#      vstd spans name these roots); the Verus toolchain there has no rust-src
#      (VI-0031);
#   2. engine/verus/test-engine.py and test-cancel.py against the stock release;
#   3. the fork's rust_verify, vir and air library tests and every sail_vi*
#      test plus `basic`, on a fresh clone of the fork commit;
#   4. scripts/verus-determinism.py --runs 3 over the fast tier's direct inputs;
#   5. ci/engine-artifact.py pack --platform macos, which rewrites
#      engine/verus/artifact-macos.json, and a strings scan of every archive
#      file for /Users/ and /home/ paths and for SAIL_SCAN_TOKENS (space
#      separated, for example the host name); a hit fails the build.
# Logs and the archive (with its .sha256 file) go to WORK/qualification.
# Then publish the archive as the fork prerelease named in the pin and run
# `published`. Python: SAIL_RELEASE_PYTHON (default python3, 3.9 or newer).
set -euo pipefail
cd "$(dirname "$0")/.."

[ "$(uname -s)-$(uname -m)" = Darwin-arm64 ] || { echo "run on an Apple-silicon Mac" >&2; exit 2; }
py="${SAIL_RELEASE_PYTHON:-python3}"
pin() { sed -n "/^\[$1\]/,/^\[/s/^$2 = \"\(.*\)\"/\1/p" "$3" | head -n1; }
fork="$(pin fork commit engine/verus/fork.toml)"
release="$(pin verus release corpus/toolchain.toml)"
upstream="$(pin verus commit corpus/toolchain.toml)"
asset_dir="$(pin assets.aarch64-apple-darwin dir corpus/toolchain.toml)"
short="${fork:0:8}"
stock="${SAIL_VERUS_HOME:-${XDG_CACHE_HOME:-$HOME/.cache}/sail/verus/$release}/$asset_dir"
step() { echo "== $(date -u +%H:%M:%S) $*"; }
die() { echo "build-macos-runtime: $*" >&2; exit 1; }

cmd_build() {
  local work="${1:-/tmp/sail-engine-$short}" q build vir
  case "$work" in "$HOME"/*) die "WORK must be outside the home directory" ;; esac
  q="$work/qualification"
  rm -rf "$work"
  mkdir -p "$work/cargo/bin" "$work/cache" "$q"
  step "fork $fork on $release"
  cargo xtask fetch-verus >"$q/fetch.log" 2>&1 || die "fetch-verus failed ($q/fetch.log)"
  [ -x "$stock/verus" ] || die "stock release not found in $stock"
  cp "$(command -v rustup)" "$work/cargo/bin/rustup"
  for proxy in cargo rustc rustdoc rustfmt; do ln -s rustup "$work/cargo/bin/$proxy"; done
  # SAIL_VERUS_HOME: the stock release stays where fetch-verus put it.
  local env=(RUSTUP_HOME="$work/rustup" CARGO_HOME="$work/cargo" SAIL_CACHE_DIR="$work/cache" SAIL_VERUS_HOME="${stock%/*}"
    CARGO_TARGET_DIR="$work/sail-target" PATH="$work/cargo/bin:$PATH")

  step "build (xtask build-verus-engine)"
  env "${env[@]}" rustup toolchain install "$(sed -n 's/^channel = "\(.*\)"/\1/p' rust-toolchain.toml)" \
    --profile minimal >"$q/engine-build.log" 2>&1
  env "${env[@]}" cargo xtask build-verus-engine --force >>"$q/engine-build.log" 2>&1 \
    || die "engine build failed ($q/engine-build.log)"
  build="$(ls -d "$work/cache/verus-src-build/$upstream"-*)"
  grep -E "verification results|built from source" "$q/engine-build.log" | tail -2
  echo "variant $(basename "$build")"
  vir="$build/verus/source/target-verus/release/vstd.vir"
  echo "vstd.vir: $(strings -a "$vir" | grep -c '/rustc/') /rustc/ spans, $(strings -a "$vir" | grep -c 'rustlib/src/rust') local"

  local exe="$build/verus/source/target-verus/release/verus"
  step "test-engine.py"
  "$py" engine/verus/test-engine.py --engine "$exe" --stock "$stock/verus" --z3 "$stock/z3" \
    --out "$q/source-fixtures" >"$q/test-engine.log" 2>&1 || die "test-engine.py failed ($q/test-engine.log)"
  tail -n 11 "$q/test-engine.log"
  step "test-cancel.py"
  "$py" engine/verus/test-cancel.py --engine "$exe" --stock "$stock/verus" --z3 "$stock/z3" \
    --out "$q/cancel-restart" >"$q/test-cancel.log" 2>&1 || die "test-cancel.py failed ($q/test-cancel.log)"
  tail -n 1 "$q/test-cancel.log"

  step "fork library and regression tests"
  local src="$work/fork"
  git clone -q --filter=blob:none "$(pin fork repo engine/verus/fork.toml)" "$src"
  git -C "$src" -c advice.detachedHead=false checkout -q "$fork"
  # Every command is checked explicitly: `set -e` does not apply in a
  # subshell whose status `||` tests.
  (
    cd "$src" || exit 1
    export "${env[@]}"
    unset CARGO_TARGET_DIR
    rustup toolchain install >/dev/null || exit 1
    (cd tools/vargo && cargo build --release -q) || exit 1
    cd source || exit 1
    export VERUS_Z3_PATH="$stock/z3" PATH="$src/tools/vargo/target/release:$PATH"
    vargo build --release >"$q/fork-build.log" 2>&1 || exit 1
    for crate in rust_verify vir air; do
      cargo test --release -p "$crate" --lib >"$q/lib-$crate.log" 2>&1 || exit 1
      grep "test result" "$q/lib-$crate.log"
    done
    tests=()
    for t in rust_verify_test/tests/sail_vi*.rs; do tests+=(--test "$(basename "$t" .rs)"); done
    vargo test --release -p rust_verify_test "${tests[@]}" --test basic >"$q/sail-vi.log" 2>&1 || exit 1
    echo "sail_vi* and basic: $(grep -c '^test result: ok' "$q/sail-vi.log") of $((${#tests[@]} / 2 + 1)) targets ok"
  ) || die "fork tests failed (logs in $q)"

  step "determinism (3 runs, fast tier direct inputs)"
  cargo build --release --locked -q -p sail-conformance
  ./target/release/sail-conformance fetch --tier fast >/dev/null
  local corpus="${SAIL_CACHE_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}/sail}/corpus" inputs=()
  while IFS= read -r f; do inputs+=("$f"); done < <(
    find "$corpus/verus-${upstream:0:12}/examples" -name '*.rs' -not -path '*/integer_ring/*' -not -path '*/scache/*' | sort
    ls "$corpus"/human-eval-verus-*/tasks/*.rs)
  local here="$PWD"
  (cd "$work" && VERUS_Z3_PATH="$stock/z3" "$py" "$here/scripts/verus-determinism.py" --runs 3 \
    --out "$q/determinism" --inputs "${inputs[@]}" -- "$exe" --crate-type=lib {} \
    --output-json --time --no-report-long-running -Z write-long-types-to-disk=no) >"$q/determinism.log" 2>&1 \
    || echo "determinism: differences found; review $q/determinism.log (Z3's own rlimit counts can vary, corpus/repros/z3-nla-rlimit-nondeterminism)"
  tail -n 1 "$q/determinism.log"

  step "pack and scan"
  "$py" ci/engine-artifact.py pack --platform macos --build "$build" \
    --archive "$q/engine-runtime.tar.gz" --pin engine/verus/artifact-macos.json >"$q/pack.log" 2>&1 \
    || die "pack failed ($q/pack.log)"
  (cd "$q" && shasum -a 256 engine-runtime.tar.gz >engine-runtime.tar.gz.sha256 && cat engine-runtime.tar.gz.sha256)
  local scan="$work/scan"
  mkdir -p "$scan" && tar -xzf "$q/engine-runtime.tar.gz" -C "$scan"
  "$py" ci/engine-artifact.py scan --root "$scan" \
    || die "the runtime path scan failed; do not publish it"
  echo "scan: clean"
  step "done: publish $q/engine-runtime.tar.gz{,.sha256} as $(sed -n 's/.*"release_tag": "\(.*\)".*/\1/p' engine/verus/artifact-macos.json), then run: $0 published"
}

cmd_published() {
  local cache rt toolchain
  toolchain="$(pin verus rust_toolchain corpus/toolchain.toml)"
  cache="$(mktemp -d /tmp/sail-engine-fetch.XXXXXX)/cache"
  rt="$("$py" ci/engine-artifact.py fetch --pin engine/verus/artifact-macos.json --cache "$cache")"
  echo "fetched and validated: $rt"
  cargo build --release --locked -q -p sail-cli -p sail-shim
  SAIL_CACHE_DIR="$cache" ./target/release/sail engine status
  SAIL_ENGINE_BUILD="$rt" SAIL_VERUS_BUILD="$rt" SAIL_CONFORMANCE_OUT="${cache%/cache}/conformance" \
    ci/run.sh conformance fast pr 2>&1 | grep -E '^conformance|^engine conformance|cache entries'
  local new="${cache%/cache}/new" bin="$rt/verus/source/target-verus/release"
  mkdir -p "$new"
  (cd "$new" && PATH="$bin:$PATH" cargo verus new --lib demo >/dev/null \
    && grep '^vstd' demo/Cargo.toml && cd demo \
    && RUSTUP_TOOLCHAIN="$toolchain" \
       PATH="$bin:$PATH" cargo verus verify 2>&1 | grep 'verification results')
}

case "${1:-}" in
  build) shift; cmd_build "$@" ;;
  published) cmd_published ;;
  *) sed -n '2,30p' "$0" | sed 's/^# \{0,1\}//'; exit 2 ;;
esac
