#!/bin/bash
# CapybaraKV cold, rotated rounds: stock, the published 0.2.0 archive, and the
# prototype's members-only lookup (alternate-bundle research mode, because the
# release index binds the published `sail`). No trace or explanation variables.
#   capy6.sh OUT ROUNDS PROTOTYPE_BIN
set -u
. ~/env.sh
out=$1 rounds=$2 proto=$3
for v in $(env | sed -n 's/^\(SAIL_[A-Z0-9_]*\)=.*/\1/p'); do unset $v; done
mkdir -p $out
now() { perl -MTime::HiRes=time -e 'printf "%.3f\n", time'; }
bundle=$(ls -d $REL/deps/*)
arms=(plain rel proto)
n=${#arms[@]}
: > $out/results.tsv
for ((r = 1; r <= rounds; r++)); do
  for ((i = 0; i < n; i++)); do
    arm=${arms[$(((r - 1 + i) % n))]}
    dir=$out/p cache=$out/c
    rm -rf $dir $cache; cp -r ~/sp/capy-rel/pristine $dir; mkdir -p $cache
    log=$out/r$r.$arm.log
    sync; sleep 2
    (
      cd $dir/capybaraKV/capybarakv
      export SAIL_CACHE_DIR=$cache RUSTUP_TOOLCHAIN=1.99.1
      case $arm in
        rel) export PATH=$REL/bin:$PATH ;;
        proto) export PATH=$proto:$PATH SAIL_VSTD_BOUNDARY_RESEARCH=1 SAIL_SHIPPED_DEPS=$bundle SAIL_SPIKE_OWNED_LOOKUP=1 ;;
      esac
      t0=$(now)
      case $arm in
        plain) cargo verus verify --no-default-features > $log 2>&1 ;;
        *) sail verify --no-default-features > $log 2>&1 ;;
      esac
      code=$?
      t1=$(now)
      stats=$(sail status --json 2>/dev/null | jq -c '.stats | [.hellos, .gets, .hits, .puts]' 2>/dev/null)
      [ $arm = plain ] && stats=-
      results=$(grep -c '^verification results::' $log)
      printf '%s\t%s\t%s\t%s\t%s\t%s\n' $r $arm $(echo "$t1 - $t0" | bc) $code "$stats" "$(grep '^verification results::' $log | tr '\n' ';')" >> $out/results.tsv
      ~/sail/scripts/sail-stop-daemon.sh >/dev/null 2>&1
    )
    tail -1 $out/results.tsv
  done
done
echo CAPY6-DONE
