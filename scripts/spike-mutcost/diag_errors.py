#!/usr/bin/env python3
"""Spike A3: one mutant's errors in every arm (Sail scoped/full, stock focus/full).

  diag_errors.py OUT_DIR LIST.json ARM:PHASE:LABEL...
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from a3_functions import by_id, errors, relog, rows  # noqa: E402


def main():
    out, listing, *specs = sys.argv[1:]
    arms = [s.split(":") for s in specs]
    got = {(a, p): by_id(rows(out, a), p) for a, p, _ in arms}
    for m in json.load(open(listing))["mutants"]:
        print("##", m["id"], m["description"])
        for a, p, label in arms:
            r = got[(a, p)].get(m["id"])
            e = errors(relog(out, a, r)) if r else None
            print(f"  {label:14} {r['outcome'] if r else '-':9}",
                  [(x[0][:48], x[1]) for x in e] if e is not None else "-")


if __name__ == "__main__":
    main()
