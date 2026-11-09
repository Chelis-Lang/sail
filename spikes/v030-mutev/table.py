#!/usr/bin/env python3
"""Spike A1: one row per mutant from `analyze.py`'s record, then the split.

    table.py RESULT.json [--tsv FILE]

Columns: id, groups, class, outcome and exit codes of the five stock runs,
whether the runs agree (exit, stderr bytes, per-function results and rlimit,
solver replies; solver input separately), the functions stock newly reports
failed and whether they are the mutated function, each error's message and
whether a span of it lies in the mutated function, and every solver answer
that is not `unsat` with its verbatim reason.
"""
import json
import sys
from collections import Counter

# Stable means: the same exit code, the same diagnostic blocks (as a
# multiset; `stderr_order_stable` reports the byte order separately), the same
# per-function results and rlimit, and the same solver replies.
STABLE = ("exit", "stderr_blocks_sha256", "verification_results", "failing", "function_results",
          "function_rlimits", "answers_digest", "not_unsat")


def row(mid, m):
    runs = m["runs"]
    oracle = m.get("oracle") or {}
    r0 = runs[0] if runs else {}
    name = m["meta"]["function"]
    new = r0.get("new_failing", [])
    same_fn = [f.rsplit("::", 1)[-1] == name for f in new]
    errors = r0.get("errors", [])
    outcome = (oracle.get("outcomes") or ["?"])[0]
    if m.get("dropped"):
        outcome = "dropped: " + m["dropped"]
    stable = all(m["identical"].get(k) for k in STABLE) and len(runs) == 5
    if outcome == "errors":
        if errors and all(e["in_mutated_function"] for e in errors) and new and all(same_fn):
            attribution = "mutated-function"
        elif errors and any(e["in_mutated_function"] for e in errors):
            attribution = "mixed"
        elif errors:
            attribution = "elsewhere"
        else:
            attribution = "no-error-printed"
    else:
        attribution = "-"
    answers = Counter()
    for k, n in r0.get("not_unsat", []):
        answers[k] += n
    return {
        "id": mid, "groups": ",".join(m.get("groups") or []), "class": m["meta"]["class"],
        "generator_mode": m["meta"]["mode"], "function": name, "outcome": outcome,
        "exits": oracle.get("exits"), "runs": len(runs), "stable": stable,
        "solver_input_stable": m["identical"].get("inputs_digest"),
        "stderr_order_stable": m["identical"].get("stderr_sha256"),
        "unstable_fields": [k for k in STABLE if not m["identical"].get(k)],
        "new_failing": new, "attribution": attribution,
        "errors": [[e["message"], e["in_mutated_function"], e["primary_in_mutated_function"]]
                   for e in errors],
        "not_unsat": dict(answers),
        "failing_rlimit": r0.get("failing_rlimit"),
    }


def main():
    data = json.load(open(sys.argv[1]))
    rows = [row(k, v) for k, v in sorted(data["mutants"].items())]
    if "--tsv" in sys.argv:
        with open(sys.argv[sys.argv.index("--tsv") + 1], "w") as f:
            cols = list(rows[0])
            f.write("\t".join(cols) + "\n")
            for r in rows:
                f.write("\t".join(json.dumps(r[c]) if not isinstance(r[c], str) else r[c]
                                  for c in cols) + "\n")
    for r in rows:
        print(r["id"], r["groups"], r["outcome"], r["exits"], "stable" if r["stable"] else
              f"UNSTABLE{r['unstable_fields']}", "input-stable" if r["solver_input_stable"] else
              "input-varies", "" if r["stderr_order_stable"] else "stderr-order-varies",
              r["attribution"], [e[0] for e in r["errors"]], r["not_unsat"],
              [f.split("::", 1)[-1] for f in r["new_failing"]] if r["attribution"] not in
              ("mutated-function", "-") else "")
    for group in ("catalogue", "exec-token", ""):
        sel = [r for r in rows if not group or group in r["groups"].split(",")]
        print(f"\n== {group or 'all'}: {len(sel)} mutants")
        print(" outcome:", dict(Counter(r["outcome"] for r in sel)))
        print(" attribution of rejected:", dict(Counter(r["attribution"] for r in sel
                                                        if r["outcome"] == "errors")))
        print(" stable across 5 runs:", sum(r["stable"] for r in sel), "of", len(sel),
              "| solver input byte-stable:", sum(bool(r["solver_input_stable"]) for r in sel),
              "| stderr byte order stable:", sum(bool(r["stderr_order_stable"]) for r in sel))
        answers = Counter()
        kinds = Counter()
        for r in sel:
            if r["outcome"] != "errors":
                continue
            for k, n in r["not_unsat"].items():
                answers[k] += n
            kinds[json.dumps(sorted({k for k in r["not_unsat"]}))] += 1
            pass
        print(" non-unsat answers (run 0, all sessions):", dict(answers))
        print(" rejected mutants by set of non-unsat answers:", dict(kinds))
        print(" diagnostic messages:", dict(Counter(e[0] for r in sel for e in r["errors"])))
    print("\noriginal failing:", {k: v["failing"] for k, v in data["originals"].items()})


main()
