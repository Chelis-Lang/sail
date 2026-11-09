#!/usr/bin/env bash
# Host: retention sizes, slice vs full session, trace perturbation.
set -uo pipefail
. ~/.cargo/env
S=~/spike; cd $S
V=$(echo ~/.cache/sail/verus/*/verus-x86-linux)
X=~/sail/target/release/examples/health_spike
for p in ironkv vest; do
  # Retention sizes under the policy: raw and zstd -3 / -19, input only.
  raw=0; z3=0; z19=0; n=0
  while read -r s; do
    f=traces-$p/$s.in.smt2
    raw=$((raw + $(stat -c %s $f))); z3=$((z3 + $(zstd -3 -c $f | wc -c))); z19=$((z19 + $(zstd -19 -c $f | wc -c))); n=$((n + 1))
  done <lists/retain-$p.txt
  echo "retain $p sessions=$n raw=$raw zstd3=$z3 zstd19=$z19"
  all=$(cat traces-$p/*.in.smt2 | wc -c); allz=$(cat traces-$p/*.in.smt2 | zstd -3 -c | wc -c)
  echo "all $p raw=$all zstd3_concatenated=$allz"
done >sizes.txt 2>&1
for p in ironkv vest; do
  $X slice $V/z3 lists/slice-$p.tsv 14 >slice-$p.tsv 2>slice-$p.err
  $X perturb $V/z3 lists/perturb-$p.tsv 14 >perturb-$p.tsv 2>perturb-$p.err
  sed "s#^#$S/traces-$p/#; s#\$#.in.smt2#" lists/retain-$p.txt >lists/full-$p.tsv
  $X perturb-full $V/z3 lists/full-$p.tsv 8 >perturb-full-$p.tsv 2>perturb-full-$p.err
done
echo REPLAY-DONE
