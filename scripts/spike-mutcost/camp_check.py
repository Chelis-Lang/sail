#!/usr/bin/env python3
"""Spike A4: a campaign arm's per-mutant classification against the naive stock loop.

  camp_check.py OUT_DIR CAMPAIGN_ARM NAIVE_ARM LIST.json [--survivors]

Campaign class of a mutant (the baseline verifies everything):
  rejected   the scoped run failed with at least one error that is not a resource limit
  rlimit     the scoped run failed with resource-limit errors only
  survived   the scoped run verified and the unscoped confirmation verified
  overturned the scoped run verified and the unscoped confirmation failed with an error
             that is not a resource limit
  survived+rlimit-elsewhere  the scoped run verified and the confirmation failed with
             resource-limit errors only
  stillborn  does not compile
Naive class: the same from the stock full run alone (rejected / rlimit / survived / stillborn).
"""
import json
import os
import statistics
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from a3_functions import RLIMIT, by_id, errors, relog, rows  # noqa: E402


def failed_class(out, arm, r):
    errs = errors(relog(out, arm, r))
    return "rlimit" if errs and all(RLIMIT.search(e[0]) for e in errs) else "rejected"


def main():
    out, camp, naive, listing = sys.argv[1:5]
    mutants = json.load(open(listing))["mutants"]
    c, n = rows(out, camp), rows(out, naive)
    scoped, confirm, stock = by_id(c, "scoped"), by_id(c, "confirm"), by_id(n, "mutant")
    table, agree, both = {}, 0, 0
    survivors, differ = [], []
    for m in mutants:
        i = m["id"]
        if i not in scoped:
            continue
        s = scoped[i]
        if s["outcome"] == "stillborn":
            cc = "stillborn"
        elif s["outcome"] == "errors":
            cc = failed_class(out, camp, s)
        elif i in confirm:
            if confirm[i]["outcome"] == "verified":
                cc = "survived"
            elif failed_class(out, camp, confirm[i]) == "rlimit":
                cc = "survived+rlimit-elsewhere"
            else:
                cc = "overturned"
        else:
            cc = "unconfirmed:" + s["outcome"]
        nc = None
        if i in stock:
            k = stock[i]
            nc = {"stillborn": "stillborn", "verified": "survived"}.get(k["outcome"]) or failed_class(out, naive, k)
            both += 1
            same = cc == nc
            agree += same
            if not same:
                differ.append((i, cc, nc, m["description"]))
        table[(cc, nc)] = table.get((cc, nc), 0) + 1
        if cc.startswith("survived"):
            survivors.append(m)
    print(f"## {camp} against {naive}: {len(scoped)} mutants, {both} with a naive result, {agree} agree\n")
    print("| campaign class | naive class | mutants |\n|---|---|---|")
    for (cc, nc), k in sorted(table.items(), key=lambda x: str(x)):
        print(f"| {cc} | {nc} | {k} |")
    for d in differ:
        print("DIFFERENT:", *d)
    sw = [r["wall_s"] for r in scoped.values() if r["outcome"] != "stillborn"]
    cw = [r["wall_s"] for r in confirm.values()]
    nw = [r["wall_s"] for r in stock.values() if r["outcome"] != "stillborn"]
    if sw:
        print(f"\nscoped run wall: median {statistics.median(sw):.2f} s, mean {statistics.mean(sw):.2f} s, "
              f"max {max(sw):.2f} s, sum {sum(sw):.1f} s (n={len(sw)})")
    if cw:
        print(f"confirmation wall: median {statistics.median(cw):.2f} s, mean {statistics.mean(cw):.2f} s, "
              f"max {max(cw):.2f} s, sum {sum(cw):.1f} s (n={len(cw)})")
    if nw:
        print(f"naive stock run wall: median {statistics.median(nw):.2f} s, mean {statistics.mean(nw):.2f} s, "
              f"max {max(nw):.2f} s, sum {sum(nw):.1f} s (n={len(nw)})")
    for r in c + n:
        if "meter" in r and "id" not in r:
            print(f"  {camp if r in c else naive} {r['phase']}: {r['meter']}"
                  + (f" scoped_wall_s={r['scoped_wall_s']}" if "scoped_wall_s" in r else ""))
    if "--survivors" in sys.argv:
        print(f"\n## Survivors of {camp} ({len(survivors)})\n")
        for m in survivors:
            print(f"### {m['id']}: {m['description']}\n\n```diff")
            for line in m["find"].splitlines():
                print("-" + line)
            for line in m["replace"].splitlines():
                print("+" + line)
            print("```\n")


if __name__ == "__main__":
    main()
