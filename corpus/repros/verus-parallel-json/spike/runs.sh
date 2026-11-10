#!/usr/bin/env bash
# runs.sh OUTROOT ARM(stock|fork) MODE(plain|tee) ROUNDS PROJ...   ; sequential within one arm.
# proj2 runs with CARGO_BUILD_JOBS=4 (the recorded reduction); others with Cargo's default.
set -u
. ~/.cargo/env
out="$1"; arm="$2"; mode="$3"; rounds="$4"; shift 4
stock=~/verus-stock/verus-x86-linux
fork=$(echo ~/engine-cache/verus-src-build/*/verus/source/target-verus/release)
case "$arm" in stock) dir=$stock ;; fork) dir=$fork ;; esac
export RUSTUP_TOOLCHAIN=1.99.1 CARGO_TERM_COLOR=never
unset RUSTC_WRAPPER RUSTC_WORKSPACE_WRAPPER RUSTFLAGS
for proj in "$@"; do
  for n in $(seq 1 "$rounds"); do
    d="$out/$mode/$arm/$proj/$n"
    [ -f "$d/exit" ] && continue
    rm -rf "$d"; mkdir -p "$d"
    prog="$dir/cargo-verus"
    if [ "$mode" = tee ]; then
      # Shadow install: the real cargo-verus bytes beside the tee as `verus`.
      shadow="$d/shadow"; mkdir -p "$shadow"
      cp "$dir/cargo-verus" "$shadow/cargo-verus"
      cp ~/spike/tee-bin "$shadow/verus"
      export SAIL_CONFORMANCE_TEE_DRIVER="$dir/verus" SAIL_CONFORMANCE_TEE_DIR="$d/captures"
      mkdir -p "$d/captures"
      prog="$shadow/cargo-verus"
    fi
    jobs=(); [ "$proj" = proj2 ] && jobs=(CARGO_BUILD_JOBS=4)
    start=$(date +%s.%N)
    (cd ~/spike/"$proj" && env "${jobs[@]}" CARGO_TARGET_DIR="${FIXED_TARGET:-$d/target}" "$prog" verus verify --workspace --all-targets \
        -- --output-json --time > "$d/stdout" 2> "$d/stderr")
    code=$?
    end=$(date +%s.%N)
    rm -rf "${FIXED_TARGET:-$d/target}" "$d/shadow"
    echo "$code" > "$d/exit"
    echo "$arm $mode $proj $n exit=$code wall=$(echo "$end - $start" | bc)"
  done
done
echo "RUNS-DONE $arm $mode $*"
