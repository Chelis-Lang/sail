#!/bin/bash
# (a) CapybaraKV stock output against the prototype's served output.
# (b) Where the fork's frontend gain on Anvil sits (--time-expanded, lite module).
# (c) Whether the focused (no-verify) vstd unit's artifacts equal a verified
#     vstd unit's artifacts under the same compiler.
set -x
. ~/env.sh
unset SAIL_PERF_VSTD SAIL_PERF_NOOPS SAIL_PERF_EDITS
out=~/sp/misc; rm -rf $out; mkdir -p $out
norm() { grep -v -E '^\s*(Compiling|Checking|Finished|Fresh|Running|Downloaded|Downloading|Locking|Updating|Adding)\b|^note\[SAIL|^WALL' "$1" | sed 's#/home/ubuntu/sp/[^ /]*/proj#<copy>#g'; }

# (a)
cp -r ~/sp/capy-rel/pristine $out/capy
( cd $out/capy/capybaraKV/capybarakv && RUSTUP_TOOLCHAIN=1.99.1 /usr/bin/time -f "WALL %e" cargo verus verify --no-default-features > $out/capy-stock.log 2>&1 )
grep WALL $out/capy-stock.log
norm $out/capy-stock.log | sed "s#$out/capy#<copy>#g" > $out/capy-stock.norm
norm ~/sp/capy-t2-on/log > $out/capy-on.norm
norm ~/sp/capy-t2-off/log > $out/capy-off.norm
wc -l $out/*.norm
diff $out/capy-stock.norm $out/capy-on.norm > $out/capy-stock-vs-on.diff; echo "stock-vs-on diff exit $? lines $(wc -l < $out/capy-stock-vs-on.diff)"
diff $out/capy-off.norm $out/capy-on.norm > $out/capy-off-vs-on.diff; echo "off-vs-on diff exit $? lines $(wc -l < $out/capy-off-vs-on.diff)"
diff <(sort $out/capy-stock.norm) <(sort $out/capy-on.norm) > $out/capy-stock-vs-on.sorted.diff; echo "sorted stock-vs-on diff exit $? lines $(wc -l < $out/capy-stock-vs-on.sorted.diff)"
head -20 $out/capy-stock-vs-on.sorted.diff

# (b)
for arm in stock fork; do
  d=$out/lite-$arm; cp -r ~/sp/timed/pristine $d
  pre=""; [ $arm = fork ] && pre="$FORKDIR:"
  ( cd $d && PATH="$pre$PATH" cargo verus focus --lib -- --verify-module vdeployment_controller::trusted::step --time-expanded > $out/lite-$arm.log 2>&1 )
  grep -A12 '^total-time' $out/lite-$arm.log
done

# (c) focused vstd artifacts (from E1's stock focus build) against a verified
# vstd built by stock `cargo verus verify` on the same Anvil copy; stop once
# vstd's dependents start.
ls -la ~/sp/e1/stock-proj/target/verus-partial/debug/deps/ | grep -i vstd
d=$out/lite-stock
( cd $d && timeout 120 cargo verus verify --lib -p verus_temporal_logic > $out/verify-vstd.log 2>&1 ); echo "verify exit $?"
tail -3 $out/verify-vstd.log
find $d/target -name 'libvstd*' -o -name 'vstd*.vir' | xargs ls -la
find $d/target ~/sp/e1/stock-proj/target -name 'libvstd*.rmeta' -o -name 'vstd*.vir' | xargs sha256sum
echo MISC-DONE
