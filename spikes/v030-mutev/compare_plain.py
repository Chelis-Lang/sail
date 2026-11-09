#!/usr/bin/env python3
"""Spike A1: is the traced stock arm the same as stock with no shim at all?

    compare_plain.py TRACED.json PLAIN.json

Per mutant, the untraced run (stock's own Z3, no Sail binary in the process
tree) against the first traced run: exit code, diagnostic blocks, Verus's
totals, the failing functions and every function's result and rlimit.
"""
import json
import sys

KEYS = ("stderr_blocks_sha256", "verification_results", "failing", "function_results",
        "function_rlimits")
traced = json.load(open(sys.argv[1]))["mutants"]
plain = json.load(open(sys.argv[2]))["mutants"]
same = 0
for mid, t in sorted(traced.items()):
    p = plain.get(mid)
    if not p or not p["runs"] or not t["runs"]:
        print(mid, "missing")
        continue
    diff = [k for k in KEYS if p["runs"][0][k] != t["runs"][0][k]]
    if (p["oracle"] or {}).get("exits", [None])[0] != (t["oracle"] or {}).get("exits", [None])[0]:
        diff.append("exit")
    if diff:
        print(mid, "DIFFERS", diff)
    else:
        same += 1
print(f"{same} of {len(traced)} mutants: the untraced stock run equals the traced one")
