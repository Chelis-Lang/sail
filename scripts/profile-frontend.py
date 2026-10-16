#!/usr/bin/env python3
"""Summarise native perf-real stage profiles: profile-frontend.py OUT_DIR...

Failed samples stay counted and prevent their group from yielding stage timings.
Missing instrumentation is displayed as unavailable, never as zero work.
"""
import pathlib, re, statistics, sys
from collections import defaultdict

TOP = [("total-time", r"^total-time:\s+(\d+)"), ("rust", r"^    rust-time:\s+(\d+)"),
       ("init-and-types", r"init-and-types:\s+(\d+)"), ("trait-conflicts", r"trait-conflicts:\s+(\d+)"),
       ("compile", r"compile-time:\s+(\d+)"), ("vir", r"vir-time:\s+(\d+)"), ("hir", r"hir-time:\s+(\d+)"),
       ("import", r"import-time:\s+(\d+)"), ("rust-to-vir", r"rust-to-vir:\s+(\d+)"),
       ("verify-crate", r"verify-crate-time:\s+(\d+)"), ("unaccounted", r"unaccounted-time:\s+(\d+)")]


def parse(log):
    t = log.read_text(errors="replace")
    d = defaultdict(float)
    for k, rx in TOP:
        for m in re.finditer(rx, t, re.M):
            d[k] += int(m.group(1)) / 1000
    for m in re.finditer(r"^    sail-front (\S+)\s+(\d+) ms", t, re.M):
        d["front:" + m.group(1)] += int(m.group(2)) / 1000
    sec = re.search(r"sail-stage-times.*?\n((?:    \S+\s+\d+ ms\n)+)", t)
    if sec:
        for m in re.finditer(r"    (\S+)\s+(\d+) ms", sec.group(1)):
            d["bucketcpu:" + m.group(1)] += int(m.group(2)) / 1000
    for m in re.finditer(r"sail-crate-stages phase \d+ threads (\d+) buckets (\d+): (.*)", t):
        d["threads"] = int(m.group(1))
        d["buckets"] += int(m.group(2))
        for k, v in re.findall(r"([a-z-]+) (\d+) ms", m.group(3)):
            d["crate:" + k] += int(v) / 1000
    return d


for out in map(pathlib.Path, sys.argv[1:]):
    rows = [l.split("\t") for l in (out / "results.tsv").read_text().splitlines()]
    groups = defaultdict(list)
    for r in rows:
        if len(r) != 19:
            raise ValueError(f"{out}: expected 19 perf-real fields, got {len(r)}")
        rnd, mode, phase, wall = r[0], r[1], r[2], float(r[3])
        kind = "noop" if phase.startswith("noop") else phase
        d = parse(out / f"r{rnd}" / f"{mode}.{phase}.log")
        d["wall"] = wall
        d["exit"] = int(r[4])
        groups[(mode, kind)].append(d)
    print(f"## {out.name}")
    keys = sorted({k for g in groups.values() for d in g for k in d},
                  key=lambda k: (k.split(":")[0] if ":" in k else "", k))
    order = ["wall"] + [k for k, _ in TOP] + [k for k in keys if ":" in k or k in ("threads", "buckets")]
    cols = sorted(groups)
    print("| | " + " | ".join(f"{m} {p} (n={len(groups[(m, p)])})" for m, p in cols) + " |")
    print("|---|" + "---:|" * len(cols))
    failures = {c: sum(d["exit"] != 0 for d in groups[c]) for c in cols}
    print("| failed samples | " + " | ".join(str(failures[c]) for c in cols) + " |")
    for k in order:
        vals = []
        for c in cols:
            if failures[c] and k != "wall":
                vals.append("failed scope")
                continue
            if any(k not in d for d in groups[c]):
                vals.append("unavailable")
                continue
            xs = [d[k] for d in groups[c]]
            vals.append(f"{statistics.median(xs):.2f}" + (f" [{min(xs):.2f}-{max(xs):.2f}]" if k == "wall" and len(xs) > 1 else ""))
        print(f"| {k} | " + " | ".join(vals) + " |")
    print()
