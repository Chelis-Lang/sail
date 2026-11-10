#!/usr/bin/env bash
# tee-series.sh ROUNDS PROJ... : stock then fork per round, same fixed target path for both arms.
rounds="$1"; shift
export FIXED_TARGET=~/spike/tee-target
for proj in "$@"; do
  for n in $(seq 1 "$rounds"); do
    ~/runs.sh ~/spike/out stock tee "$n" "$proj" | grep -v RUNS-DONE
    ~/runs.sh ~/spike/out fork tee "$n" "$proj" | grep -v RUNS-DONE
  done
done
echo "TEE-SERIES-DONE $*"
