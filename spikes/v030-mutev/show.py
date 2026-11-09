#!/usr/bin/env python3
"""Spike A1: prints one mutant's record: range, errors with spans, stderr.

    show.py RESULT.json ID [ID...]
"""
import json
import sys

data = json.load(open(sys.argv[1]))
for mid in sys.argv[2:]:
    m = data["mutants"][mid]
    r = m["runs"][0]
    print("==", mid, m["description"], m["meta"]["file"], m["meta"]["line"], "mode",
          m["meta"]["mode"], "range", m["function_lines"])
    print("new failing:", r["new_failing"], "identical:", m["identical"])
    for e in r["errors"]:
        print("  ", e["message"], e["spans"], e["in_mutated_function"])
    print("\n".join(line for line in r["stderr"].splitlines()
                    if not line.startswith("warning") and line.strip())[:2500])
