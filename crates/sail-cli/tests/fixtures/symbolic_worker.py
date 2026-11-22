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
named = "--property-proof" in args
if named:
    row["property"] = args[args.index("--property-proof") + 1]
    row["format"] = "dive-property-proof-v1"
code = 0
if mode == "repair":
    assert named and "--verify-predicate" in args and "--write-predicate" in args
    assert Path(args[args.index("--verify-predicate") + 1]).read_bytes() == b"cold"
    Path(args[args.index("--write-predicate") + 1]).write_text("repaired")
    row.update(proof="inductive-safety", failed_obligation=None)
elif "--verify-predicate" in args:
    if strategy == "occupancy":
        row["predicate_format"] = "dive-occupancy-support-v1"
    assert Path(args[args.index("--verify-predicate") + 1]).read_bytes() == b"cold"
    if mode == "hung":
        import time
        time.sleep(30)
    if mode in ("prove", "counts", "null", "missing", "unbound", "cross-format") or mode.startswith("extra-"):
        row.update(proof="inductive-safety", failed_obligation=None)
        if mode.startswith("extra-"):
            key = mode.removeprefix("extra-")
            row[key] = {"distinct_decimal": "37", "generated_decimal": None, "occupancies": 37,
                        "trace_file": None, "predicate_format": "out-of-scope", "left_on_queue": 123, "statistics": {"depth": 9}}[key]
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
        if strategy == "occupancy":
            row.update(predicate_format="dive-occupancy-support-v1", occupancies=3,
                       distinct_decimal=str(row["distinct"]), generated_decimal=str(row["generated"]))
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
if named and mode == "cold":
    for key in ("verdict", "distinct", "generated", "depth"):
        del row[key]
    row.update(proof="inductive-safety", failed_obligation=None)
if named:
    row.update(support_bits=2, support_variables=["got"], projected_integers=[], closure_sweeps=0, seconds=0.01)
    if row.get("proof"):
        row.update(transition_obligations=2 if row["proof"] == "inductive-safety" else None, reused_obligations=0)
    if mode == "null-obligations":
        row.update(proof="inductive-safety", failed_obligation=None, transition_obligations=None)
        code = 0
    if mode == "forged-time":
        row.update(proof="inductive-safety", failed_obligation=None, seconds={"statistics": {"distinct": 3}})
        code = 0
if named and mode == "unbound":
    row["config"] = args[args.index("--config") + 1]
    row["property"] = "DifferentProperty"
print(json.dumps(row))
sys.exit(code)
