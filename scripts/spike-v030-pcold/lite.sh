#!/bin/bash
# "Anvil-lite": the same package, dependency graph and frontend as the Anvil
# cold row, but a solver-light module, so the fixed per-run overheads are not
# hidden under the +-5 s variation of the dominating Z3 query.
#   lite.sh OUT ROUNDS MODULE
set -u
. ~/env.sh
unset SAIL_PERF_VSTD SAIL_PERF_NOOPS SAIL_PERF_EDITS
out=$1 rounds=$2 module=$3
mkdir -p $out
now() { perl -MTime::HiRes=time -e 'printf "%.3f\n", time'; }
arms=(plain fork-plain fork-shim sail sail-events)
n=${#arms[@]}
: > $out/results.tsv
for ((r = 1; r <= rounds; r++)); do
  for ((i = 0; i < n; i++)); do
    arm=${arms[$(((r - 1 + i) % n))]}
    dir=$out/p cache=$out/c
    rm -rf $dir $cache; cp -r ~/sp/timed/pristine $dir; mkdir -p $cache
    log=$out/r$r.$arm.log
    sync; sleep 2
    (
      cd $dir
      export SAIL_CACHE_DIR=$cache
      case $arm in
        fork-*) export PATH="$FORKDIR:$PATH" ;;
      esac
      if [ $arm = fork-shim ]; then
        export SAIL_DAEMON_SOCKET=$cache/d.sock
        sail serve --socket $SAIL_DAEMON_SOCKET > $out/r$r.daemon.log 2>&1 &
        for k in $(seq 100); do sail status --json 2>/dev/null | jq -e .running >/dev/null && break; sleep 0.05; done
        export VERUS_Z3_PATH=$(command -v sail-z3) SAIL_Z3_PATH=$FORKDIR/z3
      fi
      [ $arm = sail-events ] && export SAIL_JOB_EVENTS=$out/r$r.events.jsonl
      t0=$(now)
      case $arm in
        sail | sail-events) sail verify --output json --lib -- --verify-module $module > $log 2>&1 ;;
        *) cargo verus focus --lib -- --verify-module $module > $log 2>&1 ;;
      esac
      code=$?
      t1=$(now)
      printf '%s\t%s\t%s\t%s\n' $r $arm $(echo "$t1 - $t0" | bc) $code >> $out/results.tsv
      ~/sail/scripts/sail-stop-daemon.sh >/dev/null 2>&1
    )
    tail -1 $out/results.tsv
  done
done
echo LITE-DONE
