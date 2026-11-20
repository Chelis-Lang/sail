#!/usr/bin/env python3
"""Synthetic process protocol responses for the symbolic-check CLI regression."""
import json
import os
from pathlib import Path
import sys

mode = os.environ["FIXTURE_MODE"]
args = sys.argv[1:]
assert args[0] == "check"
assert args[args.index("--strategy") + 1] == "symbolic"
assert args[args.index("--output") + 1] == "json"
assert 0 < int(args[args.index("--time-limit") + 1].removesuffix("ms")) <= 10000
row = dict(format="dive-symbolic-report-v1", module=args[1], config=args[args.index("--config") + 1])
code = 0
if "--verify-predicate" in args:
    assert Path(args[args.index("--verify-predicate") + 1]).read_bytes() == b"cold"
    if mode in ("prove", "counts", "missing", "unbound"):
        row.update(proof="inductive-safety", failed_obligation=None)
        if mode == "counts":
            row["distinct"] = 9
        if mode == "missing":
            del row["failed_obligation"]
        if mode == "unbound":
            row["config"] = "wrong.cfg"
    else:
        row.update(proof="inconclusive", failed_obligation="Next closure")
        code = 2
else:
    Path(args[args.index("--write-predicate") + 1]).write_text(mode)
    if mode in ("cold", "refresh"):
        row.update(verdict="success", distinct=123456789123456789123456789, generated=123456789123456789123456790, depth=4)
    elif mode == "stop":
        row = dict(verdict="stopped", reason="time limit")
        code = 4
    elif mode == "bad-json":
        print("usage error")
        sys.exit(2)
    elif mode in ("violation", "unchecked"):
        row.update(verdict="invariant-violation", trace_validated=mode == "violation", depth=2,
                   ending={"kind": "invariant", "name": "Empty"}, trace={"states": ["s0", "s1"]})
        code = 1
    else:
        raise ValueError(mode)
print(json.dumps(row))
sys.exit(code)
