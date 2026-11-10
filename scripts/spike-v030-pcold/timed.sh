#!/bin/bash
set -x
. ~/env.sh
cd ~/sail
SAIL_PERF_OUT=~/sp/timed SAIL_PERF_VERUS_TIMES=0 SAIL_PERF_FORK=$FORKDIR SAIL_PERF_MODES="plain fork-plain fork-shim sail" scripts/perf-real.sh run anvil 6
echo TIMED-DONE
