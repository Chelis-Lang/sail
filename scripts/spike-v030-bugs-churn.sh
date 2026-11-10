#!/usr/bin/env bash
# SPIKE #696: does file churn in a watched ancestor directory withhold Current?
#   scripts/spike-v030-bugs-churn.sh NAME DIR PAIRS_PER_SEC JOBS
# Creates and removes PAIRS_PER_SEC files per second directly in DIR while
# JOBS ordinary Cargo jobs run in sequence on a fresh service under /tmp.
set -uo pipefail
name="$1"; dir="$2"; rate="$3"; jobs="$4"
here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
root="/tmp/c-$name"; rm -rf "$root"; mkdir -p "$root" "$dir"
bash "$here/scripts/spike-v030-bugs-env.sh" "$root" start >/dev/null
eval "$(bash "$here/scripts/spike-v030-bugs-env.sh" "$root" env)"
python3 - "$dir" "$rate" <<'PY' &
import os, sys, time
d, rate = sys.argv[1], float(sys.argv[2])
i = 0
start = time.time()
while rate > 0:
    p = os.path.join(d, f"sail-spike-churn-{os.getpid()}-{i}")
    open(p, "w").close(); os.unlink(p)
    i += 1
    ahead = i / rate - (time.time() - start)
    if ahead > 0:
        time.sleep(ahead)
PY
churn=$!
echo "== churn $name: $rate create+delete pairs/s in $dir"
python3 "$here/scripts/spike-v030-bugs.py" --sail "$SAIL" --mode cargo --count "$jobs" --work "$root/work"
kill "$churn" 2>/dev/null
bash "$here/scripts/spike-v030-bugs-env.sh" "$root" stop
