#!/usr/bin/env python3
"""Spike prototype for `sail tune` (issues #158/#244): rank isolation candidates.

    spike-tune-rank.py rank LOG [--source-root DIR] [--module SUBSTR] [--share 0.5]
    spike-tune-rank.py compare BEFORE_LOG AFTER_LOG

LOG is a file holding a `sail verify --output json` project result (the
`sail-project-result` line; other lines are ignored).

`rank` groups each requested target's functions by Verus module and separates
functions that share the module's solver process (no obligation carries
`spinoff`) from functions already in their own process. For a shared bucket it
prints each function's recorded solver time and rlimit, and proposes the
smallest prefix of functions by recorded time whose removal leaves at most
`1 - share` of the bucket's shared solver time.

`compare` reports, per function present in both results, verdict changes and
rlimit use against the limit before and after.

Not a product command: no validation runs happen here.
"""
import argparse
import json
import sys


def load(path):
    result = None
    with open(path, errors="replace") as handle:
        for line in handle:
            if line.startswith('{"kind":"sail-project-result"'):
                result = json.loads(line)
    if result is None:
        sys.exit(f"{path}: no sail-project-result line")
    return result


def functions(result):
    for target in result["targets"]:
        if not target.get("requested"):
            continue
        for crate in target["report"]["crates"]:
            for function in crate["functions"]:
                yield function


def facts(function):
    obligations = function.get("obligations") or []
    checks = [c for o in obligations for c in o["checks"]]
    used = [c["rlimit_used"] for c in checks if c.get("rlimit_used") is not None]
    limits = {c["rlimit_limit"] for c in checks if c.get("rlimit_limit")}
    # Headroom is judged on the function's largest single check.
    worst = max(
        (c["rlimit_used"] / c["rlimit_limit"] for c in checks
         if c.get("rlimit_used") is not None and c.get("rlimit_limit")),
        default=None,
    )
    return {
        "name": function["function"],
        "module": function["module"],
        "verdict": function["verdict"],
        "unstable": function["unstable"],
        "micros": function["timing"]["smt_micros"] or 0,
        "rlimit": function["timing"]["rlimit"],
        "used_sum": sum(used),
        "limit": max(limits) if limits else None,
        "worst_fraction": worst,
        "spinoff": any(o.get("spinoff") for o in obligations),
        "file": next((o["span"]["file"] for o in obligations if o.get("span")), None),
        "line": next((o["span"]["line"] for o in obligations if o.get("span")), None),
        "attributed": function.get("obligations") is not None,
        "checks": len(checks),
    }


def mark_source_isolation(rows, root):
    """The report's `spinoff` covers only ops Verus spins off inside a function
    (nonlinear, bit-vector). A function-level `spinoff_prover` attribute is not
    in the report, so read the attribute lines above each function's span."""
    cache = {}
    for row in rows:
        if not row["file"]:
            continue
        path = f"{root}/{row['file']}"
        if path not in cache:
            try:
                cache[path] = open(path, errors="replace").read().split("\n")
            except OSError:
                cache[path] = None
        lines = cache[path]
        if lines is None:
            row["source_unknown"] = True
            continue
        index = row["line"] - 2
        while index >= 0 and lines[index].lstrip().startswith(("#[", "//")):
            if "spinoff_prover" in lines[index]:
                row["spinoff"] = True
            index -= 1
        if "spinoff_prover" in lines[row["line"] - 1]:
            row["spinoff"] = True


def short(name):
    return name.rsplit("::", 1)[-1]


def rank(args):
    rows = [facts(f) for f in functions(load(args.log))]
    if args.source_root:
        mark_source_isolation(rows, args.source_root)
    modules = {}
    for row in rows:
        modules.setdefault(row["module"], []).append(row)
    order = sorted(modules, key=lambda m: -sum(r["micros"] for r in modules[m] if not r["spinoff"]))
    for module in order:
        if args.module and args.module not in module:
            continue
        shared = sorted((r for r in modules[module] if not r["spinoff"]), key=lambda r: -r["micros"])
        spun = [r for r in modules[module] if r["spinoff"]]
        total = sum(r["micros"] for r in shared)
        if total < args.min_seconds * 1e6:
            continue
        print(f"\nmodule {module}: shared bucket {total / 1e6:.1f} s over {len(shared)} functions; "
              f"{len(spun)} already isolated ({sum(r['micros'] for r in spun) / 1e6:.1f} s)")
        removed, proposal = 0, []
        for row in shared:
            if removed >= args.share * total:
                break
            proposal.append(row)
            removed += row["micros"]
        for row in shared[:args.top]:
            worst = "n/a" if row["worst_fraction"] is None else f"{100 * row['worst_fraction']:.0f}%"
            mark = "PROPOSE" if row in proposal else "       "
            print(f"  {mark} {row['micros'] / 1e6:7.2f} s  {100 * row['micros'] / max(total, 1):5.1f}%  "
                  f"rlimit {row['used_sum']:>11} worst-check {worst:>4} of limit {row['limit']}  "
                  f"{row['verdict']}{' unstable' if row['unstable'] else ''}  L{row['line']} {short(row['name'])}")
        print(f"  proposal isolates {removed / 1e6:.1f} s; predicted shared replay after: {(total - removed) / 1e6:.1f} s")


def compare(args):
    before = {r["name"]: r for r in map(facts, functions(load(args.before)))}
    after = {r["name"]: r for r in map(facts, functions(load(args.after)))}
    print(f"functions: before {len(before)}, after {len(after)}, "
          f"only-before {len(before.keys() - after.keys())}, only-after {len(after.keys() - before.keys())}")
    changed = 0

    def cell(r):
        worst = "n/a" if r["worst_fraction"] is None else f"{100 * r['worst_fraction']:.1f}%"
        return (f"{r['verdict']}{'!' if r['unstable'] else ''} {r['used_sum']} ({worst})"
                f"{' spinoff' if r['spinoff'] else ''}")

    for name in sorted(before.keys() & after.keys()):
        b, a = before[name], after[name]
        verdict = b["verdict"] != a["verdict"] or b["unstable"] != a["unstable"]
        if verdict or b["used_sum"] != a["used_sum"] or b["spinoff"] != a["spinoff"]:
            changed += 1
            print(f"  {'VERDICT ' if verdict else ''}{short(name)}: {cell(b)} -> {cell(a)}")
    print(f"functions with any verdict/rlimit/isolation change: {changed}")


def main():
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest="command", required=True)
    r = sub.add_parser("rank")
    r.add_argument("log")
    r.add_argument("--module")
    r.add_argument("--share", type=float, default=0.5)
    r.add_argument("--top", type=int, default=12)
    r.add_argument("--min-seconds", type=float, default=1.0)
    r.add_argument("--source-root", help="package root, to read existing spinoff_prover attributes")
    r.set_defaults(run=rank)
    c = sub.add_parser("compare")
    c.add_argument("before")
    c.add_argument("after")
    c.set_defaults(run=compare)
    args = parser.parse_args()
    args.run(args)


if __name__ == "__main__":
    main()
