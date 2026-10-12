#!/usr/bin/env python3
"""Prints the L11 report tables from report.json and measure.json."""
import json
import os
import statistics
import sys

root = sys.argv[1]
rep = json.load(open(os.path.join(root, "report.json")))
mes = json.load(open(os.path.join(root, "measure.json")))
rows = rep["rows"]
cats = ["agent-loops", "edits", "repair human-eval-verus", "repair vest"]


def mean(v):
    v = [x for x in v if x is not None]
    return sum(v) / len(v) if v else None


def med(v):
    v = [x for x in v if x is not None]
    return statistics.median(v) if v else None


def f(x, d=0):
    return "-" if x is None else f"{x:,.{d}f}"


print(f"near threshold: {rep['near']}")
print("| catalogue | cycles (samples) | missed buckets/cycle med (max) | setup ms/bucket p50 / p90 (offline) | live p50 / p90 | setup on critical path | A upper saving ms mean (max) | A upper % | A pred saving ms mean | B saving ms mean (max) | B % | Sail wall ms mean |")
print("|---|---|---|---|---|---|---|---|---|---|---|---|")
for c in cats:
    s = rep["summary"][c]
    rs = [r for r in rows if r["catalogue"] == c]
    up_pct = mean([100 * r["save_A_upper_ms"] / r["sail_ms"] for r in rs])
    b_pct = mean([100 * r["save_B_ms"] / r["sail_ms"] for r in rs])
    print(f"| {c} | {s['cycles']} ({s['samples']}) | {f(s['missed_buckets_per_cycle_median'])} ({s['missed_buckets_per_cycle_max']}) "
          f"| {f(s['setup_offline_ms_p50'])} / {f(s['setup_offline_ms_p90'])} | {f(s['setup_live_ms_p50'])} / {f(s['setup_live_ms_p90'])} "
          f"| {f(100 * s['critical_share_A_upper'])}% | {f(s['save_A_upper_ms_mean'])} ({f(s['save_A_upper_ms_max'])}) | {f(up_pct, 2)}% "
          f"| {f(s['save_A_f_ms_mean'])} | {f(s['save_B_ms_mean'])} ({f(s['save_B_ms_max'])}) | {f(b_pct, 2)}% | {f(s['sail_ms_mean'])} |")

print()
print("Agent-level per catalogue (means over samples; first failure = median over failing samples)")
print("| catalogue | edit-to-verdict ms: plain / Sail / A / B | first failure ms: plain / Sail / A / B | unchanged checks re-solved per cycle: plain / Sail / A / B | B fallback sessions |")
print("|---|---|---|---|---|")
for c in cats:
    s = rep["summary"][c]
    rs = [r for r in rows if r["catalogue"] == c]
    sail = s["sail_ms_mean"]
    ff = s["first_failure_ms"]
    print(f"| {c} | {f(s['plain_ms_mean'])} / {f(sail)} / {f(sail - s['save_A_f_ms_mean'])} / {f(sail - s['save_B_ms_mean'])} "
          f"| {f(ff['first_failure_ms_plain'])} / {f(ff['first_failure_ms_sail'])} / {f(ff['first_failure_ms_A'])} / {f(ff['first_failure_ms_sail'])} "
          f"| {f(s['unchanged_resolved_plain_mean'])} / {f(s['unchanged_resolved_sail_mean'], 1)} / {f(s['unchanged_resolved_sail_mean'], 1)} / {f(s['unchanged_resolved_b_mean'], 1)} "
          f"| {s['b_fallback']} of {s['b_matched']} |")

print()
print("VI-0001 reordering misses")
print("| catalogue | samples with one | re-solve tail ms median | Sail wall mean with / without | checks re-solved in them per affected sample |")
print("|---|---|---|---|---|")
b2 = mes["B2"]
for c in cats:
    rs = [r for r in rows if r["catalogue"] == c]
    w = [r["sail_ms"] for r in rs if r["permuted_misses"]]
    wo = [r["sail_ms"] for r in rs if not r["permuted_misses"]]
    print(f"| {c} | {sum(1 for r in rs if r['permuted_misses'])} of {len(rs)} | {f(med([t for r in rs for t in r['permuted_tail_ms']]))} "
          f"| {f(mean(w))} / {f(mean(wo))} | - |")
vstd = [r for r in rows if r["sequence"].startswith("edits-vstd")]
print("vstd edit samples:", [(r["round"], r["permuted_misses"], r["sail_ms"], r["plain_ms"]) for r in vstd])
print()
print("B agreement:", json.dumps(rep["b_agreement"]))
