#!/usr/bin/env python3
"""Spike A1: how two kept runs' solver sessions differ.

    session_diff.py RUN_A/trace RUN_B/trace

Pairs sessions by the sha256 of their input bytes and reports unpaired
sessions, their sizes and check answers, and the first differing input line
of the closest unpaired pair.
"""
import hashlib
import json
import sys
from pathlib import Path


def load(d):
    out = {}
    for inp in sorted(Path(d).glob("*.in.smt2")):
        data = inp.read_bytes()
        log = Path(str(inp)[: -len(".in.smt2")] + ".jsonl")
        answers = []
        for line in log.read_text().splitlines():
            try:
                r = json.loads(line)
            except ValueError:
                continue
            if r.get("type") == "check":
                answers.append(r.get("answer"))
        out.setdefault(hashlib.sha256(data).hexdigest(), []).append((inp, len(data), answers))
    return out


a, b = load(sys.argv[1]), load(sys.argv[2])
print("sessions", sum(map(len, a.values())), sum(map(len, b.values())),
      "distinct inputs", len(a), len(b), "shared", len(set(a) & set(b)))
only_a = [v for k, vs in a.items() if k not in b for v in vs]
only_b = [v for k, vs in b.items() if k not in a for v in vs]
for name, only in (("only A", only_a), ("only B", only_b)):
    for inp, size, answers in only:
        print(name, inp.name, size, len(answers), [x for x in answers if x != "unsat"])
for k in set(a) & set(b):
    if sorted(x[2] for x in a[k]) != sorted(x[2] for x in b[k]):
        print("same input, different answers", k)
if only_a and only_b:
    x = only_a[0]
    y = min(only_b, key=lambda v: abs(v[1] - x[1]))
    la, lb = x[0].read_text(errors="replace").splitlines(), y[0].read_text(errors="replace").splitlines()
    print("closest pair", x[0].name, y[0].name, len(la), len(lb))
    shown = 0
    for i, (p, q) in enumerate(zip(la, lb)):
        if p != q:
            print(i + 1, "A:", p[:200])
            print(i + 1, "B:", q[:200])
            shown += 1
            if shown >= 4:
                break
