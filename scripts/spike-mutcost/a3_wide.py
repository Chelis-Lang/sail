#!/usr/bin/env python3
"""Spike A3 at campaign scale: every mutant's scoped Sail run (a campaign arm)
against its stock full run (the naive arm), for the functions the scope selected.

  a3_wide.py OUT_DIR CAMPAIGN_ARM NAIVE_ARM LIST.json
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from a3_functions import RLIMIT, by_id, errors, relog, rows, sail_functions, verus_functions  # noqa: E402


def main():
    out, camp, naive, listing = sys.argv[1:5]
    mutants = json.load(open(listing))["mutants"]
    c, n = rows(out, camp), rows(out, naive)
    scoped, stock = by_id(c, "scoped"), by_id(n, "mutant")
    cold = next(r for r in n if r.get("phase") == "cold")
    base = verus_functions(relog(out, naive, cold))
    tally, shown = {}, []
    for m in mutants:
        i = m["id"]
        if i not in scoped or i not in stock:
            continue
        s, k = scoped[i], stock[i]
        if "stillborn" in (s["outcome"], k["outcome"]):
            key = "stillborn in both" if s["outcome"] == k["outcome"] else "stillborn in one"
            tally[key] = tally.get(key, 0) + 1
            if key == "stillborn in one":
                shown.append((i, key, s["outcome"], k["outcome"], m["description"]))
            continue
        sc, st = sail_functions(relog(out, camp, s)), verus_functions(relog(out, naive, k))
        e_sc, e_st = errors(relog(out, camp, s)), errors(relog(out, naive, k))
        sel_failed = any(v[0] for v in sc.values())
        full_failed = any((st.get(x) or [True])[0] for x in sc)
        outside = sorted(x for x, v in st.items() if v[0] and x not in sc)
        outside_rl = bool(outside) and all(RLIMIT.search(e[0]) for e in e_st if e not in e_sc)
        if sel_failed != full_failed:
            key = "MUTATED FUNCTION VERDICT DIFFERS"
        elif e_sc == e_st:
            key = "equal: verdict and errors"
        elif not outside:
            key = "verdict equal, errors differ within the mutated function"
        elif outside_rl:
            key = "verdict equal, full run adds resource-limit failures in unchanged functions"
        else:
            key = "verdict equal, full run adds other failures in unchanged functions"
        tally[key] = tally.get(key, 0) + 1
        if key != "equal: verdict and errors":
            only_sc = [e for e in e_sc if e not in e_st]
            only_st = [e for e in e_st if e not in e_sc]
            shown.append((i, key, "scoped-only", only_sc, "full-only", only_st,
                          "outside", [(x[1], base.get(x, [0, 0])[1], st[x][1]) for x in outside], m["description"]))
    print(f"## A3 wide: {camp} scoped against {naive} full\n")
    for key, v in sorted(tally.items(), key=lambda x: -x[1]):
        print(f"- {key}: {v}")
    print()
    for row in shown:
        print("-", *row)


if __name__ == "__main__":
    main()
