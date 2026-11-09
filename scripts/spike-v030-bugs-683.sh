#!/usr/bin/env bash
# SPIKE #683: one injection on a fresh cache with a vstd-dependent project.
#   scripts/spike-v030-bugs-683.sh NAME TOGGLE [VALUE]
# TOGGLE is a file under /tmp/sail-spike read by the instrumented shim:
#   provenance-delay-ms (VALUE ms), die-before-record, connect-fail, none
# Steps: p0 fresh under injection; p0 resubmitted unchanged without injection
# (target reuse); p1 fresh without injection (what the cache holds).
set -euo pipefail
name="$1"; toggle="$2"; value="${3:-1}"
here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
root="/tmp/h-$name"
rm -rf "$root" /tmp/sail-spike/shim.jsonl
rm -f /tmp/sail-spike/provenance-delay-ms /tmp/sail-spike/die-before-record /tmp/sail-spike/connect-fail
mkdir -p "$root" /tmp/sail-spike
bash "$here/scripts/spike-v030-bugs-env.sh" "$root" start >/dev/null
eval "$(bash "$here/scripts/spike-v030-bugs-env.sh" "$root" env)"
[ "$toggle" = none ] || echo "$value" >"/tmp/sail-spike/$toggle"
echo "== $name: p0 fresh, injection $toggle=$value"
python3 "$here/scripts/spike-v030-bugs.py" --sail "$SAIL" --mode cargo --count 1 --work "$root/work"
rm -f "/tmp/sail-spike/$toggle"
python3 - <<'PY'
import json, collections
c = collections.Counter(json.loads(l)["event"] for l in open("/tmp/sail-spike/shim.jsonl"))
print("shim events:", dict(c))
PY
echo "== $name: p0 unchanged resubmission, no injection"
python3 "$here/scripts/spike-v030-bugs.py" --sail "$SAIL" --mode cargo --count 1 --work "$root/work2" >/dev/null 2>&1 || true
( cd "$root/work/spike_p0" && job=$("$SAIL" verify --background | grep -oE '[0-9]{10,}-[0-9]+-[0-9]+' | head -1) \
  && for _ in $(seq 1 600); do "$SAIL" jobs status "$job" --output json | grep -q '"state": "completed"' && break; sleep 0.2; done \
  && "$SAIL" jobs result "$job" --output json | python3 -c '
import json,sys
d=json.load(sys.stdin)["value"]["result"]
def walk(v,c):
    if isinstance(v,dict):
        if "answer" in v and "source" in v:
            c["checks"]=c.get("checks",0)+1
            if v["source"] is None: c["source_null"]=c.get("source_null",0)+1
            else: c[v["source"]]=c.get(v["source"],0)+1
        for x in v.values(): walk(x,c)
    elif isinstance(v,list):
        for x in v: walk(x,c)
c={}; walk(d,c)
print("resubmit:", d["currentness"], [(t["package_id"][-30:],t["reused"]) for t in d["targets"]], c)' )
echo "== $name: the work2 project verified vstd fresh on the same cache; a third project now"
python3 "$here/scripts/spike-v030-bugs.py" --sail "$SAIL" --mode cargo --count 1 --start 5 --work "$root/work3"
bash "$here/scripts/spike-v030-bugs-env.sh" "$root" stop
