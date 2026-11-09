#!/usr/bin/env bash
# SPIKE part 2b, capture: stock pinned Verus on the fast-tier project
# human-eval-verus (corpus pin bbe93f73), three ways, logging SMT queries:
#   z3      default mode (Z3-dialect bytes, what Sail keys today)
#   compat  -V cvc5-smtlib (cvc5-compatible SMT-LIB solved by Z3)
#   cvc5    -V cvc5 with VERUS_CVC5_PATH = cvc5 1.3.4 (the version Verus requires)
# Then flattens up to 200 single-check scripts per mode.
# Output: ~/spike-out/verus-{z3,compat,cvc5}.{log,json}, ~/spike-out/vq-*/
set -euo pipefail
# shellcheck disable=SC1091
. "$HOME/.cargo/env"
out="$HOME/spike-out"
verus_home="$HOME/.cache/sail/verus/0.2026.10.18.7d82994/verus-x86-linux"
src="$HOME/spike-src/human-eval-verus"
mkdir -p "$HOME/spike-src"
if [ ! -d "$src" ]; then
  git clone -q https://github.com/secure-foundations/human-eval-verus "$src"
  git -C "$src" checkout -q bbe93f73b0a5802c438501f9a2106f9126bd2eea
fi
# Verus 0.2026.10.18 refuses any cvc5 but 1.3.4 (air/src/smt_verify.rs:195;
# the 1.1.2 named in docs/survey/verus-z3.md is stale and aborts the run).
tools="$HOME/spike-tools"
if [ ! -x "$tools/cvc5-1.3.4/bin/cvc5" ]; then
  curl -fsSL -o "$tools/cvc5-1.3.4.zip" \
    https://github.com/cvc5/cvc5/releases/download/cvc5-1.3.4/cvc5-Linux-x86_64-static.zip
  unzip -q -o "$tools/cvc5-1.3.4.zip" -d "$tools/cvc5-1.3.4.tmp"
  mv "$tools/cvc5-1.3.4.tmp/cvc5-Linux-x86_64-static" "$tools/cvc5-1.3.4"
  rmdir "$tools/cvc5-1.3.4.tmp"
fi
sha256sum "$tools/cvc5-1.3.4.zip" "$tools/cvc5-1.3.4/bin/cvc5"
export VERUS_CVC5_PATH="$tools/cvc5-1.3.4/bin/cvc5"
arms="${SPIKE_ARMS:-z3 compat cvc5}"
cd "$src"
run() {
  local name="$1"; shift
  rm -rf "$out/vlog-$name"
  local start=$SECONDS
  set +e
  "$verus_home/verus" tasks/lib.rs --crate-type=lib --log smt --log-dir "$out/vlog-$name" \
    --output-json "$@" >"$out/verus-$name.json" 2>"$out/verus-$name.log"
  local code=$?
  set -e
  echo "verus $name: exit $code, $((SECONDS - start)) s, $(find "$out/vlog-$name" -name '*.smt2' | wc -l) smt2 logs," \
    "$(cat "$out/vlog-$name"/*.smt2 2>/dev/null | grep -c '^(check-sat' || true) check-sat lines"
  grep -o '"verified": *[0-9]*\|"errors": *[0-9]*\|"encountered-vir-error": *[a-z]*' "$out/verus-$name.json" | tr '\n' ' ' || true
  echo
  grep -c '^error' "$out/verus-$name.log" || true
}
date -u
for arm in $arms; do
  case "$arm" in
    z3) run z3 ;;
    compat) run compat -V cvc5-smtlib ;;
    cvc5) run cvc5 -V cvc5 ;;
  esac
done
cd "$HOME/sail"
for name in $arms; do
  rm -rf "$out/vq-$name"
  python3 scripts/spike-cvc5/split_checks.py "$out/vq-$name" 200 "$out/vlog-$name"/*.smt2
done
date -u
echo CAPTURE-DONE
