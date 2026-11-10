#!/usr/bin/env python3
"""Spike A4: naive stock loop cost against a pooled campaign, measured or extrapolated.

  a4_cost.py OUT_DIR CAMPAIGN_ARM NAIVE_ARM LIST.json

LIST.json is the campaign's whole mutant set. When the naive arm ran only a
class-stratified prefix of it, its total is extrapolated per class:
  setup + cold + sum_c N_c * mean_c   (wall and CPU)
with a 95% interval from the per-class sample variance (finite-population
corrected, normal approximation). When it ran every mutant, the total is measured.
"""
import json
import math
import os
import statistics
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from a3_functions import rows  # noqa: E402


def estimate(samples, population, key):
    total, var = 0.0, 0.0
    for c, n_pop in population.items():
        xs = [s[key] for s in samples if s["class"] == c]
        if not xs:
            # No sample in a class: use the overall mean (stated in the output).
            xs = [s[key] for s in samples]
        mean = statistics.mean(xs)
        sd = statistics.stdev(xs) if len(xs) > 1 else 0.0
        total += n_pop * mean
        fpc = max(0.0, 1 - len(xs) / n_pop)
        var += (n_pop ** 2) * (sd ** 2) / len(xs) * fpc
    return total, 1.96 * math.sqrt(var)


def main():
    out, camp, naive, listing = sys.argv[1:5]
    mutants = json.load(open(listing))["mutants"]
    cls = {m["id"]: m["class"] for m in mutants}
    population = {}
    for m in mutants:
        population[m["class"]] = population.get(m["class"], 0) + 1
    n = rows(out, naive)
    c = rows(out, camp)
    fixed = [r for r in n if r.get("phase") in ("setup-copy", "cold")]
    samples = [{"class": cls[r["id"]], "wall": r["wall_s"], "cpu": r["meter"]["cpu_s"],
                "rss": r["meter"]["peak_rss_mb"], "outcome": r["outcome"]}
               for r in n if r.get("phase") == "mutant" and r["id"] in cls]
    fixed_wall = sum(r["meter"]["wall_s"] for r in fixed)
    fixed_cpu = sum(r["meter"]["cpu_s"] for r in fixed)
    measured = len(samples) == len(mutants)
    w, w_ci = estimate(samples, population, "wall")
    u, u_ci = estimate(samples, population, "cpu")
    naive_rss = max([s["rss"] for s in samples] + [r["meter"]["peak_rss_mb"] for r in fixed])
    tot = next(r for r in c if r.get("phase") == "total")["meter"]
    basis = "measured" if measured else f"extrapolated from {len(samples)} of {len(mutants)} (stratified)"
    nw, nu = fixed_wall + w, fixed_cpu + u
    print(f"## A4 {camp} against {naive} ({len(mutants)} mutants)\n")
    print(f"naive per-mutant wall: median {statistics.median(s['wall'] for s in samples):.2f} s, "
          f"mean {statistics.mean(s['wall'] for s in samples):.2f} s (n={len(samples)}); "
          f"setup+cold {fixed_wall:.1f} s wall, {fixed_cpu:.1f} s CPU")
    print("sampled classes:", {k: sum(1 for s in samples if s['class'] == k) for k in population})
    print(f"naive total wall: {nw:.0f} s +/- {w_ci:.0f} ({basis})")
    print(f"naive total CPU:  {nu:.0f} s +/- {u_ci:.0f}; peak RSS {naive_rss:.0f} MB")
    print(f"campaign total:   wall {tot['wall_s']:.0f} s, CPU {tot['cpu_s']:.0f} s, peak RSS {tot['peak_rss_mb']:.0f} MB")
    lo, hi = (nw - w_ci) / tot["wall_s"], (nw + w_ci) / tot["wall_s"]
    print(f"wall ratio naive/campaign: {nw / tot['wall_s']:.2f}x (95% {lo:.2f}-{hi:.2f}); "
          f"CPU ratio {nu / tot['cpu_s']:.2f}x")


if __name__ == "__main__":
    main()
