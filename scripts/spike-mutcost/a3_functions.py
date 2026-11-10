#!/usr/bin/env python3
"""Spike A3: per-function comparison of scoped and full runs, from the arms' logs.

  a3_functions.py OUT_DIR PROJECT LIST.json [--tag fix] [--survivors]

For each mutant: the functions the scoped Sail run selected (the mutated
function and any same-named function of its module), their verdict and rlimit
in the scoped run, and the same functions' result and rlimit in the Sail full
run, the stock full run (naive arm) and the unmutated baseline. Errors are read
from each run's stderr as (message, primary location).

Classification of a mutant against the baseline, per the mutated function:
  rejected   newly failed with at least one error that is not a resource limit
  rlimit     newly failed with resource-limit errors only
  survived   no new failure
  stillborn  does not compile
"""
import json
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from campaign import json_docs, read  # noqa: E402

RLIMIT = re.compile(r"rlimit|resource limit", re.I)


def rows(out, name):
    path = os.path.join(out, name, "samples.jsonl")
    return [json.loads(line) for line in open(path)] if os.path.exists(path) else []


def by_id(rs, phase):
    return {r["id"]: r for r in rs if r.get("phase") == phase and "id" in r}


def relog(out, name, r):
    """A sample's log directory under OUT_DIR (samples record container paths)."""
    return os.path.join(out, name, "logs", os.path.basename(r["log"]))


def verus_functions(log):
    """(module, function) -> [failed?, rlimit, entries] from Verus JSON on stdout."""
    fns = {}
    for d in json_docs(read(log, "stdout.txt")):
        for m in d.get("times-ms", {}).get("smt", {}).get("smt-run-module-times", []):
            for f in m.get("function-breakdown", []):
                e = fns.setdefault((m["module"], f["function"]), [False, 0, 0])
                e[0] = e[0] or not f.get("success")
                e[1] += f.get("rlimit", 0)
                e[2] += 1
    return fns


def sail_functions(log):
    """The same from a `sail verify --output json` project result."""
    fns = {}
    try:
        doc = json.loads(read(log, "stdout.txt"))
    except ValueError:
        return fns
    reports = [t.get("report") or {} for t in doc.get("targets", [])] if "targets" in doc else [doc]
    for rep in reports:
        for c in rep.get("crates", []):
            for f in c.get("functions", []):
                e = fns.setdefault((f.get("module"), f.get("function")), [False, 0, 0])
                e[0] = e[0] or f.get("verdict") != "verified"
                e[1] += (f.get("timing") or {}).get("rlimit") or 0
                e[2] += 1
    return fns


def errors(log):
    """Sorted (message, file:line:col) of every error in a run's stderr."""
    out, lines = [], read(log, "stderr.txt").splitlines()
    for i, line in enumerate(lines):
        if not line.startswith("error"):
            continue
        msg = line.split(": ", 1)[-1]
        if msg.startswith(("could not compile", "aborting due to")):
            continue
        loc = next((x.strip()[4:] for x in lines[i + 1:i + 3] if x.strip().startswith("--> ")), "")
        out.append((msg, loc))
    return sorted(out)


def main():
    out, project, listing = sys.argv[1:4]
    tag = sys.argv[sys.argv.index("--tag") + 1] if "--tag" in sys.argv else "fix"
    mutants = json.load(open(listing))["mutants"]
    a3n, nvn = f"{project}-a3-{tag}", f"{project}-naive"
    a3, naive = rows(out, a3n), rows(out, nvn)
    scoped, full, twin, stock = by_id(a3, "scoped"), by_id(a3, "full"), by_id(a3, "twin"), by_id(naive, "mutant")
    base_row = next((r for r in a3 if r.get("phase") == "full-A"), None)
    base = verus_functions(relog(out, a3n, base_row)) if base_row else {}
    cold_row = next((r for r in naive if r.get("phase") == "cold"), None)
    stock_base = verus_functions(relog(out, nvn, cold_row)) if cold_row else {}

    print(f"## A3 {project}: mutated function, scoped against full (Sail `{tag}` build; stock = naive arm)\n")
    print("| mutant | selected fns | baseline | twin scoped | sail scoped | sail full | stock full | "
          "rlimit base / scoped / full / stock | errors scoped = sail full = stock full | "
          "failing outside scope (sail full / stock) | class | scoped=full |")
    print("|---|---|---|---|---|---|---|---|---|---|---|---|")
    tally, classes, unstable = {}, {}, []
    survivors = []
    for m in mutants:
        i = m["id"]
        if i not in scoped or i not in full:
            continue
        sc_log, fu_log = relog(out, a3n, scoped[i]), relog(out, a3n, full[i])
        sc, fu = sail_functions(sc_log), verus_functions(fu_log)
        tw = sail_functions(relog(out, a3n, twin[i])) if i in twin else {}
        st = verus_functions(relog(out, nvn, stock[i])) if i in stock else {}
        e_sc, e_fu = errors(sc_log), errors(fu_log)
        e_st = errors(relog(out, nvn, stock[i])) if i in stock else None
        still = "stillborn" in (scoped[i]["outcome"], full[i]["outcome"])
        keys = sorted(sc) or sorted(tw)

        def word(fns, table=None):
            if table is not None and not table:
                return "-"
            got = [(table or {}).get(k) for k in keys] if table is not None else fns
            if any(g is None for g in got):
                return "missing"
            return "failed" if any(g[0] for g in got) else "verified"

        def rl(table):
            return sum((table.get(k) or [0, 0])[1] for k in keys)

        b, t, s, f, k = word(None, base), word(None, tw), word(None, sc), word(None, fu), word(None, st)
        outside_fu = sorted(x for x, v in fu.items() if v[0] and x not in sc)
        outside_st = sorted(x for x, v in st.items() if v[0] and x not in sc)
        if still:
            cls = "stillborn"
            same = "stillborn" if scoped[i]["outcome"] == full[i]["outcome"] else "DIFFERENT"
        else:
            newly = s == "failed" and b == "verified"
            only_rl = bool(e_sc) and all(RLIMIT.search(x[0]) for x in e_sc)
            cls = ("rlimit" if only_rl else "rejected") if newly else "survived"
            same = "equal" if (s == f and e_sc == e_fu) else ("verdict-equal" if s == f else "DIFFERENT")
        if cls == "survived" and not outside_fu:
            survivors.append(m)
        tally[same] = tally.get(same, 0) + 1
        classes[cls] = classes.get(cls, 0) + 1
        if not still and rl(sc) != rl(fu):
            unstable.append((i, rl(sc), rl(fu)))
        errs = "-" if still else f"{'yes' if e_sc == e_fu else 'NO'} / {'-' if e_st is None else 'yes' if e_fu == e_st else 'NO'} ({len(e_sc)})"
        print(f"| {i}{' (near rlimit)' if m.get('near_rlimit') else ''} | {len(keys)} | {b} | {t} | {s} | {f} | {k} | "
              f"{rl(base)} / {rl(sc)} / {rl(fu)} / {rl(st)} | {errs} | {len(outside_fu)} / {len(outside_st)} | {cls} | {same} |")
    print(f"\nscoped against full: {tally}; classes: {classes}")
    print(f"mutants whose mutated-function rlimit differs between the scoped and the full run: {len(unstable)}"
          + "".join(f"\n  {i}: scoped {a}, full {b}" for i, a, b in unstable))
    # Unchanged functions: does a mutant move the rlimit of functions it does not touch?
    moved, total = 0, 0
    worst = (0.0, None)
    for m in mutants:
        i = m["id"]
        if i not in stock or stock[i]["outcome"] == "stillborn":
            continue
        st = verus_functions(relog(out, nvn, stock[i]))
        sel = set(sail_functions(relog(out, a3n, scoped[i]))) if i in scoped else set()
        for key, v in st.items():
            if key in sel or key not in stock_base:
                continue
            total += 1
            b0 = stock_base[key][1]
            if v[1] != b0:
                moved += 1
                rel = abs(v[1] - b0) / max(b0, 1)
                if rel > worst[0]:
                    worst = (rel, (i, key, b0, v[1]))
    print(f"\nunchanged functions across stock full mutant runs: {moved} of {total} (function, mutant) pairs "
          f"report a different rlimit than the stock baseline; largest relative change {worst[0]:.4f} {worst[1]}")
    if "--survivors" in sys.argv:
        print("\n## Survivors (scoped and full agree, nothing fails anywhere)\n")
        for m in survivors:
            print(f"### {m['id']}: {m['description']}\n\n```diff")
            for line in m["find"].splitlines():
                print("-" + line)
            for line in m["replace"].splitlines():
                print("+" + line)
            print("```\n")


if __name__ == "__main__":
    main()
