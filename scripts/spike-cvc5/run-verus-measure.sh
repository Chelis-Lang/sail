#!/usr/bin/env bash
# SPIKE part 2b, measurement over the 200 flattened Verus obligations per
# mode produced by run-verus-capture.sh. Sequential; run under the lock.
# Output: ~/spike-out/vq-*.json and ~/spike-out/verus-measure.log
set -euo pipefail
tools="$HOME/spike-tools"
out="$HOME/spike-out"
z3="$HOME/.cache/sail/verus/0.2026.10.18.7d82994/verus-x86-linux/z3"
cvc5_rt="$tools/cvc5-1.4.2/bin/cvc5"   # the runtime pin
cvc5_vs="$tools/cvc5-1.3.4/bin/cvc5"   # the version Verus requires
carcara="$tools/carcara/bin/carcara"
m=scripts/spike-cvc5/measure.py
# The arguments Verus passes cvc5 (docs/survey/verus-z3.md, "cvc5 instead gets").
vflags=(--extra=--produce-models --extra=--quant-dsplit=none --extra=--no-cbqi --extra=--user-pat=strict)
pflags=(--proof-extra=--produce-models --proof-extra=--quant-dsplit=none --proof-extra=--no-cbqi --proof-extra=--user-pat=strict)
{
  date -u
  for name in z3 compat cvc5; do
    echo "vq-$name: $(find "$out/vq-$name" -name '*.smt2' | wc -l) scripts, $(cat "$out/vq-$name"/*.smt2 | wc -c) bytes, sha256 $(cat "$out/vq-$name"/*.smt2 | sha256sum | cut -d' ' -f1)"
  done
  # Same obligations in the same order in every mode?
  diff <(cd "$out/vq-z3" && ls) <(cd "$out/vq-cvc5" && ls) >/dev/null && echo "z3/cvc5 script names identical" || echo "z3/cvc5 script names DIFFER"
  diff <(cd "$out/vq-compat" && ls) <(cd "$out/vq-cvc5" && ls) >/dev/null && echo "compat/cvc5 script names identical" || echo "compat/cvc5 script names DIFFER"

  # 1. Z3-dialect bytes (what Sail keys today): Z3 control, then cvc5 as is.
  python3 $m solve --solver z3 --bin "$z3" --out "$out/vq-z3.z3.json" "$out/vq-z3"
  python3 $m solve --solver cvc5 --bin "$cvc5_rt" --extra=--rlimit-per=3333330 "${vflags[@]}" \
    --out "$out/vq-z3.cvc5-1.4.2.json" "$out/vq-z3"

  # 2. cvc5-compatible SMT-LIB solved by Z3 (Verus's -V cvc5-smtlib bytes).
  python3 $m solve --solver z3 --bin "$z3" --out "$out/vq-compat.z3.json" "$out/vq-compat"

  # 3. The bytes Verus sends cvc5, solved by both cvc5 versions with Verus's arguments.
  python3 $m solve --solver cvc5 --bin "$cvc5_vs" "${vflags[@]}" --out "$out/vq-cvc5.cvc5-1.3.4.json" "$out/vq-cvc5"
  python3 $m solve --solver cvc5 --bin "$cvc5_rt" "${vflags[@]}" --out "$out/vq-cvc5.cvc5-1.4.2.json" "$out/vq-cvc5"
  python3 $m solve --solver cvc5 --bin "$cvc5_rt" "${vflags[@]}" --out "$out/vq-cvc5.cvc5-1.4.2.again.json" "$out/vq-cvc5"

  # 4. Certificates over the cvc5 bytes: finest granularity, RARE rewrites checked.
  python3 $m cert --cvc5 "$cvc5_rt" --carcara "$carcara" --timeout 120 --out "$out/vq-cvc5.cert-dsl.json" \
    "${pflags[@]}" --proof-extra=--proof-granularity=dsl-rewrite \
    --carcara-extra=--allow-int-real-subtyping --carcara-extra=--expand-let-bindings \
    --carcara-extra=--rare-file --carcara-extra="$tools/carcara/rewrites.eo" "$out/vq-cvc5"
  # And with cvc5's own recommended Carcara flags (unknown rules become holes).
  python3 $m cert --cvc5 "$cvc5_rt" --carcara "$carcara" --timeout 120 --out "$out/vq-cvc5.cert-default.json" \
    "${pflags[@]}" \
    --carcara-extra=--allow-int-real-subtyping --carcara-extra=--expand-let-bindings \
    --carcara-extra=--ignore-unknown-rules "$out/vq-cvc5"

  python3 $m report "$out"/vq-*.json
  echo "--- agreement, Z3 on Z3 bytes vs Z3 on compat bytes vs cvc5 1.3.4 vs cvc5 1.4.2 (x2) on cvc5 bytes"
  python3 scripts/spike-cvc5/analyze.py agree "$out/vq-z3.z3.json" "$out/vq-compat.z3.json" \
    "$out/vq-cvc5.cvc5-1.3.4.json" "$out/vq-cvc5.cvc5-1.4.2.json" "$out/vq-cvc5.cvc5-1.4.2.again.json"
  echo "--- Z3 bytes: Z3 vs cvc5 1.4.2 as is"
  python3 scripts/spike-cvc5/analyze.py agree "$out/vq-z3.z3.json" "$out/vq-z3.cvc5-1.4.2.json"
  date -u
  echo VERUS-MEASURE-DONE
} >"$out/verus-measure.log" 2>&1
