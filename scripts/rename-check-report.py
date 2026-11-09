#!/usr/bin/env python3
"""Summarises a `sail-bench rename-check` result file (spike tooling).

Per mutation family (seed, shuffle, rename), against the baseline replay of
the same session:
- solver error replies, infrastructure failures, unpaired batches;
- answer flips per (check, configuration) and distinct checks that flip;
- effort spread: the ratio of each configuration's rlimit-count delta to the
  baseline's, as quantiles over all (check, configuration) pairs.
"""

import json
import statistics
import sys
from collections import defaultdict


def quantile(sorted_values, q):
    if not sorted_values:
        return None
    at = min(len(sorted_values) - 1, int(q * (len(sorted_values) - 1) + 0.5))
    return sorted_values[at]


def main():
    data = json.load(open(sys.argv[1]))
    configs = data["configs"]
    sessions = data["sessions"]
    print(f"machine: {data['machine']}")
    print(f"solver: {data['solver']}")
    identical = sum(1 for s in sessions if s["identity_identical"] is True)
    different = [s["id"] for s in sessions if s["identity_identical"] is False]
    refused = [(s["id"], s["refused"]) for s in sessions if s["refused"]]
    print(
        f"sessions: {len(sessions)}; identity byte-identical: {identical}; "
        f"different: {len(different)}; refused: {len(refused)}"
    )
    print(
        "bytes: {}; commands: {}; identity-rewritten tokens: {}".format(
            sum(s["bytes"] for s in sessions),
            sum(s["commands"] for s in sessions),
            sum(s["identity_tokens"] for s in sessions),
        )
    )
    totals = defaultdict(int)
    for s in sessions:
        for key, value in (s["plan"] or {}).items():
            totals[key] += value
    print(f"plan totals over sessions: {dict(totals)}")
    for item in different[:10] + refused[:10]:
        print(f"  problem: {item}")

    baseline = {}
    for run in data["runs"]:
        if configs[run["config"]] == "baseline":
            baseline[run["session"]] = run
    checks = sum(len(r["answers"]) for r in baseline.values())
    base_answers = defaultdict(int)
    for r in baseline.values():
        for a in r["answers"]:
            base_answers[a] += 1
    print(f"baseline replay: {len(baseline)} sessions, {checks} checks, answers {dict(base_answers)}")

    families = defaultdict(
        lambda: {
            "configs": set(),
            "runs": 0,
            "infra": [],
            "error_runs": 0,
            "errors": 0,
            "error_samples": [],
            "unpaired": 0,
            "compared": 0,
            "flips": 0,
            "missing": 0,
            "flip_checks": set(),
            "flip_kinds": defaultdict(int),
            "ratios": [],
            "ms": 0,
        }
    )
    for run in data["runs"]:
        label = configs[run["config"]]
        fam = families[label.split("=")[0]]
        fam["configs"].add(label)
        fam["runs"] += 1
        fam["ms"] += run["elapsed_ms"]
        if run["infra"]:
            fam["infra"].append((sessions[run["session"]]["id"], label, run["infra"]))
        if run["errors"]:
            fam["error_runs"] += 1
            fam["errors"] += len(run["errors"])
            if len(fam["error_samples"]) < 5:
                fam["error_samples"].append((sessions[run["session"]]["id"], label, run["errors"][0][:300]))
        fam["unpaired"] += run["unpaired_batches"]
        base = baseline.get(run["session"])
        if base is None or run["infra"]:
            continue
        for i, b in enumerate(base["answers"]):
            a = run["answers"][i] if i < len(run["answers"]) else None
            if a is None or b is None:
                fam["missing"] += 1
                continue
            fam["compared"] += 1
            if a != b:
                fam["flips"] += 1
                fam["flip_checks"].add((run["session"], i))
                fam["flip_kinds"][f"{b}->{a}"] += 1
            rb = base["rlimit"][i] if i < len(base["rlimit"]) else None
            ra = run["rlimit"][i] if i < len(run["rlimit"]) else None
            if rb and ra:
                fam["ratios"].append(ra / rb)

    for name in sorted(families):
        fam = families[name]
        ratios = sorted(fam["ratios"])
        print(f"\n== {name} ({len(fam['configs'])} configurations, {fam['runs']} runs, {fam['ms'] / 1000:.0f} s solver wall)")
        print(
            f"   infra failures {len(fam['infra'])}; runs with solver errors {fam['error_runs']} "
            f"({fam['errors']} error replies); unpaired batches {fam['unpaired']}"
        )
        for sample in fam["infra"][:5] + fam["error_samples"]:
            print(f"   sample: {sample}")
        if name == "baseline":
            continue
        rate = fam["flips"] / fam["compared"] if fam["compared"] else float("nan")
        print(
            f"   answers compared {fam['compared']}; flips {fam['flips']} ({rate:.6%}); "
            f"distinct checks flipping {len(fam['flip_checks'])}; missing {fam['missing']}; "
            f"kinds {dict(fam['flip_kinds'])}"
        )
        for session, i in sorted(fam["flip_checks"])[:20]:
            print(f"   flip: {sessions[session]['id']} check {i}")
        if ratios:
            logs_sd = statistics.pstdev([__import__("math").log(r) for r in ratios])
            outside = sum(1 for r in ratios if r > 1.5 or r < 1 / 1.5)
            print(
                "   effort ratio vs baseline: n {} min {:.3f} p01 {:.3f} p10 {:.3f} p50 {:.3f} "
                "p90 {:.3f} p99 {:.3f} max {:.3f}; sd(log ratio) {:.4f}; outside [1/1.5, 1.5]: {} ({:.3%})".format(
                    len(ratios),
                    ratios[0],
                    quantile(ratios, 0.01),
                    quantile(ratios, 0.10),
                    quantile(ratios, 0.50),
                    quantile(ratios, 0.90),
                    quantile(ratios, 0.99),
                    ratios[-1],
                    logs_sd,
                    outside,
                    outside / len(ratios),
                )
            )


if __name__ == "__main__":
    main()
