#!/usr/bin/env bash
# build-verus.sh NAME REPO COMMIT : source build as xtask build-verus-source --interactive-engine does
set -euo pipefail
name=$1 repo=$2 commit=$3
. ~/.cargo/env
unset RUSTC_WRAPPER RUSTFLAGS RUSTUP_TOOLCHAIN CARGO_TARGET_DIR || true
export CARGO_TERM_PROGRESS_WHEN=never GIT_TERMINAL_PROMPT=0
z3=$HOME/z3/z3
if [ ! -x "$z3" ]; then
  mkdir -p ~/z3dl && cd ~/z3dl
  curl -sSfLO https://github.com/Z3Prover/z3/releases/download/z3-4.16.0/z3-4.16.0-arm64-glibc-2.38.zip
  sha256sum z3-4.16.0-arm64-glibc-2.38.zip
  python3 -c 'import zipfile,sys; zipfile.ZipFile("z3-4.16.0-arm64-glibc-2.38.zip").extractall("x")'
  mkdir -p ~/z3 && install -m 0755 x/z3-4.16.0-arm64-glibc-2.38/bin/z3 "$z3"
  sha256sum "$z3"; "$z3" --version
fi
d=$HOME/vsrc/$name
t() { local label=$1; shift; local s=$(date +%s.%N); "$@"; local e=$(date +%s.%N); printf 'TIMING %s %s %.1f s\n' "$name" "$label" "$(echo "$e - $s" | bc)"; }
if [ ! -d "$d/.git" ]; then
  mkdir -p "$d" && cd "$d" && git init -q && git remote add origin "$repo"
  git fetch -q --depth 1 origin "$commit" && git checkout -q --detach FETCH_HEAD
fi
cd "$d"; test "$(git rev-parse HEAD)" = "$commit"
t toolchain rustup toolchain install
cd "$d/tools/vargo"; t vargo cargo build --release
export PATH=$d/tools/vargo/target/release:$PATH VERUS_Z3_PATH=$z3
cd "$d/source"
t create-manifest cargo run --locked -p cargo-verus-toolchains --bin create-manifest -- --write-to-dir cargo-verus/toolchain-manifests
t vargo-build vargo build --release
t vstd-cargo-verus target-verus/release/cargo-verus build --release --manifest-path vstd/Cargo.toml
ls -la target-verus/release | head -60
sha256sum target-verus/release/z3
echo DONE $name
