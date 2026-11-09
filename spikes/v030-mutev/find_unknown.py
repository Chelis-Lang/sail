#!/usr/bin/env python3
"""Spike A1: where a run's non-unsat solver answers are.

    find_unknown.py RUN/trace [CONTEXT_LINES]

For every check that did not answer `unsat`: the session, the check's
position, and the input lines before its `(check-sat)`.
"""
import json
import sys
from pathlib import Path

ctx = int(sys.argv[2]) if len(sys.argv) > 2 else 12
for log in sorted(Path(sys.argv[1]).glob("*.jsonl")):
    checks = [json.loads(line) for line in log.read_text().splitlines()]
    checks = [c for c in checks if c.get("type") == "check"]
    bad = [c for c in checks if c.get("answer") != "unsat"]
    if not bad:
        continue
    lines = Path(str(log)[: -len(".jsonl")] + ".in.smt2").read_text(errors="replace").splitlines()
    at = [i for i, line in enumerate(lines) if line.strip() == "(check-sat)"]
    print("==", log.name, len(checks), "checks;", [(c["seq"], c["answer"]) for c in bad])
    for c in bad:
        i = at[c["seq"]]
        # The last named thing before the check: Verus's query comments are
        # not sent, so show the nearest declarations and assertions.
        print(f"-- check {c['seq']} at input line {i + 1}")
        for line in lines[max(0, i - ctx): i + 1]:
            print("   ", line[:180])
