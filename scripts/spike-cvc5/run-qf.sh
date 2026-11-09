#!/usr/bin/env bash
# SPIKE part 2a: 200 generated quantifier-free runtime-style queries.
# Z3 and cvc5 answers, then Alethe proofs checked by the pinned Carcara in
# two configurations. Sequential; run under `flock ~/spike.lock`.
# Output: ~/spike-out/qf-*.json and ~/spike-out/qf.log
set -euo pipefail
tools="$HOME/spike-tools"
out="$HOME/spike-out"
cvc5="$tools/cvc5-1.4.2/bin/cvc5"
carcara="$tools/carcara/bin/carcara"
z3="$HOME/.cache/sail/verus/0.2026.10.18.7d82994/verus-x86-linux/z3"
m=scripts/spike-cvc5/measure.py
{
  date -u
  [ -d "$out/qf" ] || python3 scripts/spike-cvc5/gen_qf.py "$out/qf" 200
  echo "queries: $(find "$out/qf" -name '*.smt2' | wc -l), corpus sha256 $(cat "$out"/qf/*.smt2 | sha256sum | cut -d' ' -f1)"
  sha256sum "$cvc5" "$carcara" "$z3" "$tools/carcara/rewrites.eo"
  python3 $m solve --solver z3 --bin "$z3" --extra rlimit=2000000 --out "$out/qf-z3.json" "$out/qf"
  # Three cvc5 passes for answer determinism under the runtime's argv.
  for pass in 1 2 3; do
    python3 $m solve --solver cvc5 --bin "$cvc5" --extra=--rlimit-per=2000000 \
      --out "$out/qf-cvc5-$pass.json" "$out/qf"
  done
  # A: cvc5's default proof granularity, Carcara flags from cvc5's own
  #    contrib/get-carcara-checker (unknown rules count as holes).
  python3 $m cert --cvc5 "$cvc5" --carcara "$carcara" --out "$out/qf-cert-default.json" \
    --carcara-extra=--allow-int-real-subtyping --carcara-extra=--expand-let-bindings \
    --carcara-extra=--ignore-unknown-rules "$out/qf"
  # B: finest granularity with RARE rewrite steps checked against the
  #    rewrites.eo shipped in the pinned Carcara source; unknown rules fail.
  python3 $m cert --cvc5 "$cvc5" --carcara "$carcara" --out "$out/qf-cert-dsl.json" \
    --proof-extra=--proof-granularity=dsl-rewrite \
    --carcara-extra=--allow-int-real-subtyping --carcara-extra=--expand-let-bindings \
    --carcara-extra=--rare-file --carcara-extra="$tools/carcara/rewrites.eo" "$out/qf"
  python3 $m report "$out/qf-z3.json" "$out"/qf-cvc5-?.json "$out/qf-cert-default.json" "$out/qf-cert-dsl.json"
  date -u
  echo QF-DONE
} >"$out/qf.log" 2>&1
