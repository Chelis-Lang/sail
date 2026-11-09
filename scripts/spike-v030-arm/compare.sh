#!/usr/bin/env bash
# Direct stock-vs-fork comparison on aarch64. Exact stdout/stderr/exit per crate root.
set -uo pipefail
. ~/.cargo/env
unset RUSTC_WRAPPER RUSTFLAGS RUSTUP_TOOLCHAIN VERUS_Z3_PATH || true
out=$HOME/cmp; rm -rf "$out"; mkdir -p "$out"
stock=$HOME/vsrc/stock/source/target-verus/release/verus
fork=$HOME/vsrc/fork/source/target-verus/release/verus
he=$HOME/corpus/human-eval-verus
if [ ! -d "$he/.git" ]; then
  mkdir -p "$he" && cd "$he" && git init -q && git remote add origin https://github.com/secure-foundations/human-eval-verus
  git fetch -q --depth 1 origin bbe93f73b0a5802c438501f9a2106f9126bd2eea && git checkout -q --detach FETCH_HEAD
fi
one() { # arm label dir file args...
  local arm=$1 label=$2 dir=$3; shift 3
  local bin=$stock; [ "$arm" = fork ] && bin=$fork
  local s e rc
  s=$(date +%s.%N)
  (cd "$dir" && "$bin" "$@" >"$out/$label.$arm.stdout" 2>"$out/$label.$arm.stderr"); rc=$?
  e=$(date +%s.%N)
  echo "$rc" >"$out/$label.$arm.exit"
  printf '%s %s rc=%s %.2f s\n' "$label" "$arm" "$rc" "$(echo "$e - $s" | bc)" >>"$out/times.txt"
}
# human-eval-verus, three rounds each, alternating
for r in 1 2 3; do
  for arm in stock fork; do one $arm "human-eval.r$r" "$he" tasks/lib.rs --crate-type=lib; done
done
# human-eval-verus with JSON output and timing (verdict details)
for arm in stock fork; do one $arm human-eval.json "$he" tasks/lib.rs --crate-type=lib --output-json --time; done
# verus-examples: every examples/*.rs and one level down, minus the corpus excludes
ex=$HOME/vsrc/stock/examples
cd "$ex"
find . -name '*.rs' | sed 's|^\./||' | grep -v -E '^(integer_ring|scache)/' | sort >"$out/examples.list"
while read -r f; do
  label="ex.$(echo "$f" | tr '/' '_')"
  for arm in stock fork; do one $arm "$label" "$ex" "$f" --crate-type=lib; done
done <"$out/examples.list"
# summary
cd "$out"
same=0; diff=0
for f in *.stock.stdout; do
  b=${f%.stock.stdout}
  if cmp -s "$b.stock.stdout" "$b.fork.stdout" && cmp -s "$b.stock.stderr" "$b.fork.stderr" && cmp -s "$b.stock.exit" "$b.fork.exit"; then
    same=$((same+1))
  else diff=$((diff+1)); echo "DIFFERENT $b"; fi
done
echo "SUMMARY same=$same different=$diff"
echo "exit codes (stock): $(cat *.stock.exit | sort | uniq -c | tr '\n' ' ')"
echo COMPARE-DONE
