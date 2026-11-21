#!/usr/bin/env python3
"""Synthetic process protocol responses for the symbolic-check CLI regression."""
import json
import os
from pathlib import Path
import sys

mode = os.environ["FIXTURE_MODE"]
args = sys.argv[1:]
assert args[0] == "check"
strategy = args[args.index("--strategy") + 1]
assert strategy == os.environ["FIXTURE_STRATEGY"]
if strategy == "occupancy":
    assert args[args.index("--max-orbits") + 1] == "34500000"
assert args[args.index("--output") + 1] == "json"
assert 0 < int(args[args.index("--time-limit") + 1].removesuffix("ms")) <= 10000
row = dict(format=f"dive-{strategy}-report-v1", module=args[1], config=args[args.index("--config") + 1])
code = 0
if "--verify-predicate" in args:
    assert Path(args[args.index("--verify-predicate") + 1]).read_bytes() == b"cold"
    if mode == "hung":
        import time
        time.sleep(30)
    if mode in ("prove", "counts", "null", "missing", "unbound", "cross-format"):
        row.update(proof="inductive-safety", failed_obligation=None)
        if mode == "cross-format":
            row["format"] = "dive-symbolic-report-v1"
        if mode == "counts":
            row["distinct"] = 9
        if mode == "null":
            row["distinct"] = None
        if mode == "missing":
            del row["failed_obligation"]
        if mode == "unbound":
            row["config"] = "wrong.cfg"
    else:
        row.update(proof="inconclusive", failed_obligation="Next closure")
        code = 2
else:
    Path(args[args.index("--write-predicate") + 1]).write_text(mode)
    if mode in ("cold", "refresh", "contradictory"):
        row.update(verdict="success", distinct=123456789123456789123456789, generated=123456789123456789123456790, depth=4)
        if mode == "contradictory":
            row.update(failed_obligation="Empty", trace_validated=True, ending={"kind": "deadlock"})
    elif mode == "stop":
        row = dict(verdict="stopped", reason="time limit")
        code = 4
    elif mode == "bad-json":
        print("usage error")
        sys.exit(2)
    elif mode == "invalid-count":
        row.update(verdict="success", distinct=False, generated="unknown", depth={})
    elif mode in ("violation", "unchecked", "invalid-trace", "duplicate-var"):
        row.update(verdict="invariant-violation", trace_validated=mode != "unchecked", depth=2,
                   ending={"kind": "invariant", "name": "Empty"}, trace={"format":"dive-trace", "version":1, "variables":["got"],
                   "states":[{"values":{"got":0}}, {"values":{"got":1}}],
                   "ending":{"kind":"invariant", "name":"Empty"}})
        if mode == "duplicate-var":
            row["trace"]["variables"] = ["got", "got"]
            for state in row["trace"]["states"]:
                state["values"]["foreign"] = 0
        if mode == "invalid-trace":
            row["trace"] = False
        code = 1
    else:
        raise ValueError(mode)
print(json.dumps(row))
sys.exit(code)
