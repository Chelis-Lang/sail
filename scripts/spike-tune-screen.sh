#!/usr/bin/env bash
# Spike prototype for `sail tune` validation (issues #158/#244): try isolation
# variants in one owned project copy with a warm Sail cache.
#
#   spike-tune-screen.sh OUT PRISTINE PKG FILE 'CARGO_ARGS' 'VERUS_ARGS' VARIANT...
#
# PRISTINE is a prepared project copy (for example a perf-real.sh `pristine`
# directory), PKG the package directory inside it, FILE the source file to
# patch (relative to PRISTINE). Each VARIANT is NAME:LINE[,LINE...]: the `fn`
# lines that receive an inline `#[verifier::spinoff_prover]`, so no line moves.
# A LINE prefixed `FILE2@` patches another file (`FILE2@LINE`).
#
# The copy is verified once unpatched (`base`), then once per variant, each
# from the pristine source. Functions outside the patched module hit the
# cache, and an isolated function's own process is keyed by its own bytes, so
# it is solved once across variants. Every run's JSON result and wall time are
# kept: OUT/NAME.log, OUT/walls.tsv. Compare with spike-tune-rank.py.
#
# Needs sail on PATH and SAIL_VERUS_BUILD (or an installed runtime).
set -euo pipefail
out=$1 pristine=$2 pkg=$3 file=$4 cargs=$5 vargs=$6
shift 6
here="$(cd "$(dirname "$0")" && pwd)"
mkdir -p "$out"
out="$(cd "$out" && pwd)"
rm -rf "$out/proj" "$out/cache"
cp -r "$pristine" "$out/proj"
export SAIL_CACHE_DIR="$out/cache"
mkdir -p "$SAIL_CACHE_DIR"
trap '"$here/sail-stop-daemon.sh" >/dev/null 2>&1 || true' EXIT
now() { perl -MTime::HiRes=time -e 'printf "%.3f\n", time'; }

run() { # NAME
  local t0 t1 code=0
  t0=$(now)
  # shellcheck disable=SC2086
  (cd "$out/proj/$pkg" && sail verify --output json $cargs -- $vargs) >"$out/$1.log" 2>&1 || code=$?
  t1=$(now)
  printf '%s\t%s\t%s\n' "$1" "$(awk -v a="$t0" -v b="$t1" 'BEGIN { printf "%.1f", b - a }')" "$code" | tee -a "$out/walls.tsv"
}

: >"$out/walls.tsv"
run base
for probe in ${TUNE_PROBES:-}; do
  cp "$pristine/$file" "$out/proj/$file"
  sed -i "${probe##*=}s/\$/ assert(true);/" "$out/proj/$file"
  run "base.${probe%%=*}"
done
cp "$pristine/$file" "$out/proj/$file"
for variant in "$@"; do
  name=${variant%%:*}
  cp "$pristine/$file" "$out/proj/$file"
  restore=("$file")
  IFS=, read -ra lines <<<"${variant#*:}"
  for entry in "${lines[@]}"; do
    target=$file line=$entry
    case "$entry" in *@*)
      target=${entry%%@*} line=${entry##*@}
      case " ${restore[*]} " in *" $target "*) ;; *) cp "$pristine/$target" "$out/proj/$target"; restore+=("$target") ;; esac ;;
    esac
    sed -n "${line}p" "$out/proj/$target" | grep -q 'fn ' || { echo "no fn at $target:$line" >&2; exit 1; }
    sed -i "${line}s/^\(\s*\)/\1#[verifier::spinoff_prover] /" "$out/proj/$target"
  done
  run "$name"
  # Optional edit probes: TUNE_PROBES="NAME=LINE ...". Each appends
  # `assert(true);` to LINE of FILE (the line holding a function body's
  # opening brace, in a body that does not start with `hide(...)`) on the
  # variant's source alone, so no later line moves and isolated functions
  # keep their bytes, and verifies against the variant's now-warm cache: the
  # wall time of that one edit.
  cp "$out/proj/$file" "$out/variant.src"
  for probe in ${TUNE_PROBES:-}; do
    cp "$out/variant.src" "$out/proj/$file"
    sed -i "${probe##*=}s/\$/ assert(true);/" "$out/proj/$file"
    run "$name.${probe%%=*}"
  done
  for f in "${restore[@]}"; do cp "$pristine/$f" "$out/proj/$f"; done
done
