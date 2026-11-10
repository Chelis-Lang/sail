#!/bin/bash
# E1/E2: unit timeline of the focused Anvil cold build, then a seeded rerun.
set -x
. ~/env.sh
unset SAIL_PERF_VSTD SAIL_PERF_NOOPS SAIL_PERF_EDITS
ts() { perl -MTime::HiRes=time -ne 'BEGIN{$|=1;$t=time} printf "%9.3f %s", time-$t, $_'; }
run() { # NAME DIR PATHPREFIX
  local name=$1 dir=$2 pre=$3
  ( cd $dir && export PATH="$pre$PATH" && which cargo-verus &&
    /usr/bin/time -f "WALL %e" cargo verus focus --lib --timings -- --verify-module vdeployment_controller 2>&1 | ts ) > ~/sp/e1/$name.log 2>&1
}
rm -rf ~/sp/e1; mkdir -p ~/sp/e1
for arm in stock fork; do
  d=~/sp/e1/$arm-proj; cp -r ~/sp/timed/pristine $d
  pre=""; [ $arm = fork ] && pre="$FORKDIR:"
  run $arm-cold $d "$pre"
  find $d -name 'cargo-timing*.html' | head -3
  find $d -type d -name .fingerprint
  cp "$(find $d -name 'cargo-timing.html' | head -1)" ~/sp/e1/$arm-cold-timing.html
  # E2: keep only the shipped vstd closure's units; everything else rebuilds.
  keep=$(grep '^name = ' $MAIN/deps/*/Cargo.lock | sed 's/name = "\(.*\)"/\1/' | sort -u | tr '\n' ' ')
  echo KEEP=$keep
  for fp in $(find $d -type d -name .fingerprint); do
    for u in $fp/*; do
      b=$(basename $u); n=${b%-*}
      case " $keep " in *" $n "*) ;; *) rm -rf $u ;; esac
    done
    ls $fp | sed 's/-[0-9a-f]*$//' | sort | uniq -c
  done
  run $arm-seeded $d "$pre"
  cp "$(find $d -name 'cargo-timing.html' | head -1)" ~/sp/e1/$arm-seeded-timing.html
done
grep -h WALL ~/sp/e1/*.log
echo E1-DONE
