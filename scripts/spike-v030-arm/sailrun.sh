#!/usr/bin/env bash
set -uo pipefail
. ~/.cargo/env
unset RUSTC_WRAPPER RUSTFLAGS RUSTUP_TOOLCHAIN VERUS_Z3_PATH || true
sail=$HOME/sail/target/release/sail
out=$HOME/sailrun; rm -rf "$out"; mkdir -p "$out"
he=$HOME/corpus/human-eval-verus
run() { # label dir cmd...
  local label=$1 dir=$2; shift 2
  local s e rc
  s=$(date +%s.%N)
  (cd "$dir" && "$@" >"$out/$label.stdout" 2>"$out/$label.stderr"); rc=$?
  e=$(date +%s.%N)
  printf '%s rc=%s %.2f s\n' "$label" "$rc" "$(echo "$e - $s" | bc)" | tee -a "$out/times.txt"
}
echo "== standalone human-eval-verus tasks/lib.rs"
run he.sail.cold "$he" "$sail" verify tasks/lib.rs -- --crate-type=lib
for i in 1 2 3; do run he.sail.warm$i "$he" "$sail" verify tasks/lib.rs -- --crate-type=lib; done
run he.sail.json "$he" "$sail" verify tasks/lib.rs --output json -- --crate-type=lib
for f in cold warm1 warm2 warm3; do
  cmp -s "$out/he.sail.$f.stdout" ~/cmp/human-eval.r1.stock.stdout && echo "$f stdout == stock" || echo "$f stdout DIFFERS from stock"
  cmp -s "$out/he.sail.$f.stderr" ~/cmp/human-eval.r1.stock.stderr && echo "$f stderr == stock" || echo "$f stderr DIFFERS from stock"
done
"$sail" status 2>&1 | head -30
echo "== cargo project (sail new)"
rm -rf ~/demo; cd ~ && "$sail" new demo 2>&1 | tail -3
if [ -d ~/demo ]; then
  run demo.sail.cold ~/demo "$sail" verify
  run demo.sail.warm1 ~/demo "$sail" verify
  run demo.sail.warm2 ~/demo "$sail" verify
  export PATH=$HOME/.cache/sail/verus/0.2026.10.18.7d82994/verus-arm64-linux:$PATH
  run demo.stock.cold ~/demo cargo verus verify
  run demo.stock.warm ~/demo cargo verus verify
  for f in demo.sail.cold demo.sail.warm1 demo.stock.cold; do echo "--- $f"; tail -6 "$out/$f.stdout"; tail -8 "$out/$f.stderr"; done
fi
echo SAILRUN-DONE
