#!/usr/bin/env python3
"""Tables for the v0.3.0 A3+A4 spike from the arms' samples.jsonl.

  analyze.py OUT_DIR PROJECT LIST.json
"""
import json
import os
import statistics
import sys


def rows(out, name):
    path = os.path.join(out, name, "samples.jsonl")
    if not os.path.exists(path):
        return []
    return [json.loads(line) for line in open(path)]


def by_id(rs, phase):
    return {r["id"]: r for r in rs if r.get("phase") == phase and "id" in r}


def phase(rs, name):
    return next((r for r in rs if r.get("phase") == name), None)


def in_scope(failed, m):
    """Failed entries of the full run that the scoped selector would select."""
    module = m["file"].split("/src/", 1)[-1][:-3].replace("/", "::")
    return [f for f in failed if f.split("#")[0] == module and f.split("#")[1].endswith("::" + m["function"])]


def main():
    out, project, listing = sys.argv[1:4]
    mutants = json.load(open(listing))["mutants"]
    naive = rows(out, f"{project}-naive")
    sscoped = rows(out, f"{project}-sscoped")
    a3 = rows(out, f"{project}-a3-fix")
    n_full, s_sc = by_id(naive, "mutant"), by_id(sscoped, "mutant")
    a_sc, a_full, a_twin = by_id(a3, "scoped"), by_id(a3, "full"), by_id(a3, "twin")

    print(f"## A3 {project}: per-mutant verdicts\n")
    print("| mutant | class | function | stock full | failing in/out of scope | stock scoped | sail scoped | sail full | scoped=full |")
    print("|---|---|---|---|---|---|---|---|---|")
    counts = {"equal": 0, "scoped-misses": 0, "other": 0, "stillborn": 0}
    sail_vs_stock_scoped = sail_vs_stock_full = compared = 0
    for m in mutants:
        i = m["id"]
        nf, ss, asc, af = n_full.get(i, {}), s_sc.get(i, {}), a_sc.get(i, {}), a_full.get(i, {})
        failed = nf.get("failed", [])
        inside = in_scope(failed, m)
        full, scoped = nf.get("outcome"), asc.get("outcome")
        if full == "stillborn":
            verdict = "stillborn"
        elif full == scoped:
            verdict = "equal"
        elif full == "errors" and scoped == "verified":
            verdict = "scoped-misses"
        else:
            verdict = "other"
        counts[verdict] += 1
        if asc and ss:
            compared += 1
            sail_vs_stock_scoped += asc.get("outcome") == ss.get("outcome")
            sail_vs_stock_full += af.get("outcome") == full
        print(f"| {i} | {m['class']} | {m['function']} | {full} | {len(inside)}/{len(failed) - len(inside)} | "
              f"{ss.get('outcome')} | {scoped} | {af.get('outcome')} | {verdict} |")
    print(f"\ncounts: {counts}; sail scoped = stock scoped: {sail_vs_stock_scoped}/{compared}; "
          f"sail full = stock full: {sail_vs_stock_full}/{compared}")
    twins = [r for r in a_twin.values()]
    print(f"baseline twins (sail scoped, unmutated): {sum(r['outcome'] == 'verified' for r in twins)}/{len(twins)} verified; "
          f"selected entries per scope: {sorted(r['reported'] for r in twins)}")

    def med(rs_, key="wall_s"):
        v = [r[key] for r in rs_ if r.get("outcome") != "stillborn"]
        return f"{statistics.median(v):.1f} s (n={len(v)})" if v else "-"

    print(f"\n## {project}: per-mutant wall, serial, non-stillborn medians\n")
    print(f"- stock full: {med(n_full.values())}\n- stock scoped (focus): {med(s_sc.values())}")
    print(f"- sail scoped: {med(a_sc.values())}\n- sail full (shared cache): {med(a_full.values())}")

    print(f"\n## {project}: second copy at another path (one daemon, one cache)\n")
    for name in ("full-A", "full-second-copy", "full-second-copy-again", "warm-scoped-A", "warm-scoped-second-copy"):
        r = phase(a3, name)
        if r:
            c = r["cache"]
            rate = f"{100 * c['hits'] / c['gets']:.2f}%" if c["gets"] else "-"
            print(f"- {name}: wall {r['wall_s']} s, gets {c['gets']}, hits {c['hits']} ({rate}), puts {c['puts']}")

    print(f"\n## A4 {project}: end to end\n")
    print("| arm | wall s | CPU s | peak RSS MB (max phase) | vs naive wall |")
    print("|---|---|---|---|---|")
    base = (phase(naive, "total") or {}).get("meter", {}).get("wall_s")
    for name in ("naive", "sscoped", "camp-asshipped", "camp-fix", "camp-unscoped-fix"):
        rs_ = rows(out, f"{project}-{name}")
        t = phase(rs_, "total")
        if not t:
            continue
        peak = max(r["meter"]["peak_rss_mb"] for r in rs_ if "meter" in r)
        ratio = f"{base / t['meter']['wall_s']:.2f}x" if base else "-"
        print(f"| {name} | {t['meter']['wall_s']} | {t['meter']['cpu_s']} | {peak} | {ratio} |")
        for r in rs_:
            if "meter" in r and "id" not in r and r["phase"] not in ("total",):
                extra = f", survivors {r['survivors']}" if "survivors" in r else ""
                cache = r.get("cache") or {}
                hits = f", cache {cache.get('hits')}/{cache.get('gets')}" if "gets" in cache else ""
                print(f"|   {r['phase']} | {r['meter']['wall_s']} | {r['meter']['cpu_s']} | {r['meter']['peak_rss_mb']} | {extra}{hits} |")
        conf = [r for r in rs_ if r.get("phase") == "confirm"]
        if conf:
            ok = sum(r["outcome"] == "verified" for r in conf)
            print(f"|   confirmations | {len(conf)} runs, {ok} confirmed survivors, "
                  f"{len(conf) - ok} overturned | sum of walls {sum(r['wall_s'] for r in conf):.1f} | | |")
        muts = [r for r in rs_ if r.get("phase") in ("mutant", "scoped", "unscoped")]
        if muts:
            print(f"|   per-mutant runs | {len(muts)} | sum of walls {sum(r['wall_s'] for r in muts):.1f} | | |")


if __name__ == "__main__":
    main()
