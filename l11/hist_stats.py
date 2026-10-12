#!/usr/bin/env python3
"""Summarises `sail-bench history --json` for L11: rlimit spread and use."""
import json
import sys

LIMIT = 30_000_000  # Verus's default per-query rlimit in these captures


def pct(v, p):
    v = sorted(v)
    if not v:
        return None
    k = (len(v) - 1) * p
    lo = int(k)
    hi = min(lo + 1, len(v) - 1)
    return v[lo] + (v[hi] - v[lo]) * (k - lo)


import glob
import os

args = sys.argv[1:]
for i in range(0, len(args), 2):
    path, troot = args[i], args[i + 1]
    limits = {}
    for f in glob.glob(os.path.join(troot, "r0", "*", "*", "fresh", "*.jsonl")):
        sid = os.path.basename(f)[:-6]
        for line in open(f):
            x = json.loads(line)
            if x.get("type") == "check" and x.get("rlimit") and x["rlimit"].isdigit():
                limits[(sid, x["seq"])] = int(x["rlimit"])
    r = json.load(open(path))
    checks = r.get("examples") or []
    if not checks:
        print(path, "keys:", list(r)[:20])
        continue
    ratios, used, ups = [], [], []
    for c in checks:
        a, b = c.get("rlimit_in_session"), c.get("rlimit_fresh")
        if a and b:
            ratios.append(max(a, b) / min(a, b))
            ups.append(b / a)
        lim = limits.get((c["id"], c["seq"]))
        if a is not None and lim:
            used.append(a / lim)
    n = len(checks)
    print(path)
    print(f"  limits joined {len(used)}; distinct limits {sorted(set(limits.values()))[:8]}")
    print(f"  checks {n}; rlimit compared {len(ratios)}; differ {sum(1 for x in ratios if x != 1)}")
    print("  spread max/min p50 %.3f p90 %.3f p99 %.3f max %.2f" % (
        pct(ratios, .5), pct(ratios, .9), pct(ratios, .99), max(ratios)))
    print("  fresh/in-session p1 %.3f p99 %.3f" % (pct(ups, .01), pct(ups, .99)))
    print("  used/limit p50 %.5f p90 %.5f p99 %.4f max %.4f" % (
        pct(used, .5), pct(used, .9), pct(used, .99), max(used)))
    for t in (0.001, 0.01, 0.027, 0.05, 0.1, 0.25, 0.5):
        print(f"  in-session used > {t:.3f} of limit: {sum(1 for u in used if u > t)}")
    worst = sorted(((max(c['rlimit_in_session'], c['rlimit_fresh']) / min(c['rlimit_in_session'], c['rlimit_fresh']), c['rlimit_in_session'] / limits.get((c['id'], c['seq']), LIMIT), c['id'], c['seq'])
                    for c in checks if c.get('rlimit_in_session') and c.get('rlimit_fresh')), reverse=True)[:5]
    print("  worst spreads (ratio, used/limit, session, check):", [(round(a, 2), round(b, 5), s, q) for a, b, s, q in worst])
