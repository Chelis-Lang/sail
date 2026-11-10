#!/bin/bash
set -x
. ~/env.sh
cd ~/sail
echo FORKDIR=$FORKDIR; ls $FORKDIR | head -30; which sail cargo-verus verus sail-conformance; sail --version
SAIL_PERF_OUT=~/sp/diag SAIL_PERF_DIAG=1 SAIL_PERF_FORK=$FORKDIR SAIL_PERF_MODES="plain fork-plain sail" scripts/perf-real.sh run anvil 1
echo DIAG-DONE
