#!/usr/bin/env bash
# SPIKE #696: control and candidate fixes on one binary (toggle files), for a
# preempted new-project preparation, a cancelled first job, and file churn in
# a watched ancestor directory.
#   SAIL_SPIKE_BIN=<dir of the prototype binaries> scripts/spike-v030-bugs-696-fix.sh ROOT [CHURN_PAIRS_PER_SEC]
set -uo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$here"
root="$1"
mkdir -p "$root"
bash scripts/spike-v030-bugs-env.sh "$root" start >/dev/null
eval "$(bash scripts/spike-v030-bugs-env.sh "$root" env)"
rm -f /tmp/sail-spike/fix-696 /tmp/sail-spike/fix-absent-drain
churn="${2:-0}"
if [ "$churn" != 0 ]; then
  # Both arms run with the first-observation fix, so the drain is the only variable.
  touch /tmp/sail-spike/fix-696
  for arm in control fix; do
    [ "$arm" = fix ] && touch /tmp/sail-spike/fix-absent-drain
    python3 - "$root" "$churn" <<'PY' &
import os, sys, time
d, rate = sys.argv[1], float(sys.argv[2])
i = 0
start = time.time()
while True:
    p = os.path.join(d, f"sail-spike-churn-{os.getpid()}-{i}")
    open(p, "w").close(); os.unlink(p)
    i += 1
    ahead = i / rate - (time.time() - start)
    if ahead > 0:
        time.sleep(ahead)
PY
    pid=$!
    echo "== churn $arm: $churn create+delete pairs/s in $root"
    python3 scripts/spike-v030-bugs.py --sail "$SAIL" --mode cargo --count 3 --start 500 --work "$root/churn-$arm"
    kill "$pid"
  done
  rm -f /tmp/sail-spike/fix-absent-drain /tmp/sail-spike/fix-696
  echo CHURN-FIX-DONE
  exit 0
fi
echo "== control: preempted preparation"
python3 scripts/spike-v030-bugs-696.py --sail "$SAIL" --work "$root/control" --gaps 1,3 --repeat 2
echo "== control: cancelled first job"
python3 scripts/spike-v030-bugs-696.py --sail "$SAIL" --work "$root/control-cancel" --gaps 5 --repeat 2 --cancel
touch /tmp/sail-spike/fix-696
echo "== fix: preempted preparation"
python3 scripts/spike-v030-bugs-696.py --sail "$SAIL" --work "$root/fix" --gaps 1,3 --repeat 2
echo "== fix: cancelled first job"
python3 scripts/spike-v030-bugs-696.py --sail "$SAIL" --work "$root/fix-cancel" --gaps 5 --repeat 2 --cancel
rm -f /tmp/sail-spike/fix-696
echo FIX-696-DONE
