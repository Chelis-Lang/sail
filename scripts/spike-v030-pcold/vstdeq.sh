#!/bin/bash
# Does stock's focused (no-verify) vstd unit emit the same rmeta/vir bytes as
# its verified vstd unit, at identical absolute paths? Control: focus twice.
set -x
. ~/env.sh
unset SAIL_PERF_VSTD SAIL_PERF_NOOPS SAIL_PERF_EDITS
out=~/sp/vstdeq; rm -rf $out; mkdir -p $out
cp -r ~/sp/timed/pristine $out/proj
cd $out/proj
T=$out/t
sums() { (cd $T/verus-partial/debug/deps && sha256sum libvstd-*.rmeta libvstd-*.vir vstd-*.vir 2>/dev/null; ls -la | grep -i vstd); }
for n in focus1 focus2; do
  rm -rf $T
  CARGO_TARGET_DIR=$T cargo verus focus --lib -- --verify-module vdeployment_controller::trusted::step > $out/$n.log 2>&1; echo "$n exit $?"
  sums > $out/$n.sums; cat $out/$n.sums
done
rm -rf $T
# Verified vstd at the same absolute paths; stop after vstd's dependents start.
( CARGO_TARGET_DIR=$T/verus-partial cargo verus verify --lib > $out/verify.log 2>&1 & pid=$!
  for i in $(seq 240); do grep -q 'verus_temporal_logic' $out/verify.log && break; sleep 0.5; done
  sleep 1; kill $pid; pkill -f 'rust_verify' ; wait $pid 2>/dev/null )
grep -E 'vstd|verification results|verus_temporal' $out/verify.log | head
sums > $out/verify.sums; cat $out/verify.sums
diff <(cut -d' ' -f1,3 $out/focus1.sums | grep -v '^-') <(cut -d' ' -f1,3 $out/focus2.sums | grep -v '^-') && echo CONTROL-EQUAL
diff <(cut -d' ' -f1,3 $out/focus1.sums | grep -v '^-') <(cut -d' ' -f1,3 $out/verify.sums | grep -v '^-') && echo VERIFY-EQUAL
echo VSTDEQ-DONE
