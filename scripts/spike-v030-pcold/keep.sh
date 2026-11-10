#!/bin/bash
# One cold run of CapybaraKV or IronKV, keeping the cache so the native
# observation directories can be inspected afterwards.
#   keep.sh NAME SAILBIN capy|iron [ENV=VALUE...]
set -x
. ~/env.sh
name=$1 bin=$2 project=$3; shift 3
out=~/sp/$name
rm -rf $out; mkdir -p $out
case $project in
  capy) cp -r ~/sp/capy-rel/pristine $out/proj; cd $out/proj/capybaraKV/capybarakv; cargs=(--no-default-features) ;;
  iron) cp -r ~/sp/iron-rel/pristine $out/proj; cd $out/proj/ironsht; cargs=() ;;
esac
export PATH=$bin:$PATH
for v in $(env | sed -n 's/^\(SAIL_[A-Z0-9_]*\)=.*/\1/p'); do unset $v; done
export SAIL_CACHE_DIR=$out/c RUSTUP_TOOLCHAIN=1.99.1 SAIL_SHIPPED_TRACE=$out/shipped SAIL_EXPLAIN=1
for kv in "$@"; do export "$kv"; done
which sail sail-z3
/usr/bin/time -f "WALL %e" sail verify "${cargs[@]}" > $out/log 2>&1
tail -4 $out/log | cut -c1-400
sail status --json | jq -c '.stats | {hellos, gets, hits, puts}'
~/sail/scripts/sail-stop-daemon.sh
ls $out/shipped 2>/dev/null | sed 's/-.*//' | sort | uniq -c
for f in $out/shipped/vstd-*.json; do jq -c '{outcome, key}' $f; done
echo KEEP-DONE
