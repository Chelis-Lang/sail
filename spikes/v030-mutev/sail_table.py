#!/usr/bin/env python3
"""Spike A1: Sail's report per mutant beside stock's.

    sail_table.py SAIL_OUT_DIR TRACED.json [--json FILE]

Per mutant: Sail's exit code; whether its diagnostic blocks equal stock's
(as a multiset); the functions it does not report verified with their
labels; whether those are the functions stock reports failed; and, per such
function, every check's answer and verbatim `reason-unknown`, split into
ordinary and advisory (recommends) obligations.
"""
import json
import sys
from collections import Counter
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from analyze import block_digest  # noqa: E402

out = Path(sys.argv[1])
summary = json.loads((out / "summary.json").read_text())
traced = json.load(open(sys.argv[2]))["mutants"]
rows = {}
labels = Counter()
answers = Counter()
agree = Counter()
withheld = Counter()
attributed = Counter()
for mid, s in sorted(summary.items()):
    if mid in ("original", "reverted"):
        fns = [f["function"] for t in s.get("targets", []) for f in t["not_verified"]]
        print(mid, "exit", s.get("exit"), "not verified:", fns, "wall_s", s.get("wall_s"))
        continue
    stock = traced[mid]["runs"][0]
    stock_exit = (traced[mid]["oracle"] or {}).get("exits", [None])[0]
    stderr = (out / "logs" / mid / "stderr.txt").read_text(errors="replace")
    every = [f for t in s.get("targets", []) for f in t["not_verified"]]
    # In a run with a failure Sail labels every other function of the target
    # `unknown` (the target is not complete); only `failed` ones are compared.
    fns = [f for f in every if f["verdict"] == "failed"]
    withheld[len(every) - len(fns) > 0] += 1
    stock_fns = sorted(f.split("#", 1)[1].split("::", 1)[1] if "::" in f else f
                       for f in stock["failing"])
    row = {
        "exit": s.get("exit"), "stock_exit": stock_exit,
        "diagnostics_equal_stock": block_digest(stderr) == stock["stderr_blocks_sha256"],
        "attribution": [t.get("attribution", {}).get("status") for t in s.get("targets", [])],
        "complete": [t.get("complete") for t in s.get("targets", [])],
        "stock_failing": stock_fns,
        "functions": [],
        "other_functions_labelled_unknown": len(every) - len(fns),
        "wall_s": s.get("wall_s"),
    }
    for f in fns:
        checks = Counter()
        advisory = Counter()
        for o in f["obligations"]:
            for c in o["checks"]:
                key = f'{c["answer"]} {c["reason_unknown"] or ""}'.strip()
                (advisory if o["advisory"] else checks)[key] += 1
        row["functions"].append({
            "function": f["function"], "fn_mode": f["fn_mode"], "verdict": f["verdict"],
            "establishment": f["establishment"], "evidence_tier": f["evidence_tier"],
            "unstable": f["unstable"], "checks": dict(checks), "advisory_checks": dict(advisory),
            "provenance_source": (f["provenance"] or {}).get("source"),
            "obligations": len(f["obligations"]),
            "counterexamples": sum(o["has_counterexample"] for o in f["obligations"]),
        })
        labels[(f["verdict"], f["establishment"], f["evidence_tier"])] += 1
        attributed["failed function has attributed checks" if checks else
                   "failed function has no attributed checks"] += 1
        for k, n in checks.items():
            if not k.startswith("unsat"):
                answers[k] += n
    sail_fns = sorted(f["function"] for f in fns if f["verdict"] != "verified")
    # Stock names a function `module::path`; Sail prefixes the crate.
    row["same_functions_as_stock"] = len(sail_fns) == len(stock_fns) and all(
        a.endswith(b) or b.endswith(a.split("::", 1)[-1]) for a, b in zip(sail_fns, stock_fns))
    agree[(row["exit"] == stock_exit, row["diagnostics_equal_stock"])] += 1
    rows[mid] = row
    print(mid, "exit", row["exit"], "stock", stock_exit, "diag=stock", row["diagnostics_equal_stock"],
          "attr", row["attribution"], "others-unknown", row["other_functions_labelled_unknown"],
          [(f["function"].split("::")[-1], f["verdict"], f["establishment"],
            f["provenance_source"], f["checks"]) for f in row["functions"]],
          "" if row["same_functions_as_stock"] else f"STOCK FAILING: {stock_fns} SAIL: {sail_fns}")
print("\n(exit equals stock, diagnostics equal stock):", dict(agree))
print("labels of failed functions:", dict(labels))
print("runs where Sail labels the other functions unknown:", dict(withheld))
print("solver answers attributed to the failed function:", dict(attributed))
print("same failed functions as stock:", dict(Counter(r["same_functions_as_stock"] for r in rows.values())))
print("report attribution status:", dict(Counter(str(r["attribution"]) for r in rows.values())))
print("non-unsat answers on ordinary obligations:", dict(answers))
if "--json" in sys.argv:
    Path(sys.argv[sys.argv.index("--json") + 1]).write_text(
        json.dumps(rows, indent=1, sort_keys=True) + "\n")
