#!/usr/bin/env bash
# strace.sh ARM PROJ : one plain run with every write to fd 1 traced (all processes).
set -u
. ~/.cargo/env
arm="$1"; proj="$2"
stock=~/verus-stock/verus-x86-linux
fork=$(echo ~/engine-cache/verus-src-build/*/verus/source/target-verus/release)
case "$arm" in stock) dir=$stock ;; fork) dir=$fork ;; esac
export RUSTUP_TOOLCHAIN=1.99.1 CARGO_TERM_COLOR=never
unset RUSTC_WRAPPER RUSTC_WORKSPACE_WRAPPER RUSTFLAGS
d=~/spike/strace/$arm/$proj; rm -rf "$d"; mkdir -p "$d"
jobs=(); [ "$proj" = proj2 ] && jobs=(CARGO_BUILD_JOBS=4)
(cd ~/spike/"$proj" && env "${jobs[@]}" CARGO_TARGET_DIR="$d/target" \
  strace -f --seccomp-bpf -y -e trace=write -e trace-fds=1 -e signal=none -s 4000000 -o "$d/trace" \
  "$dir/cargo-verus" verus verify --workspace --all-targets -- --output-json --time > "$d/stdout" 2> "$d/stderr")
echo $? > "$d/exit"
rm -rf "$d/target"
echo "STRACE-DONE $arm $proj exit=$(cat $d/exit)"
