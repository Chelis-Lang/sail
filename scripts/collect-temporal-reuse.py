#!/usr/bin/env python3
"""Collect the paired S7 JSONL samples from perf-temporal-reuse.py into citable JSON."""
import argparse
import json
import math
from pathlib import Path


def rows(path):
    return [json.loads(line) for line in path.read_text().splitlines()]


def series(root, suite, candidate, runs):
    """Check result parity before folding duplicate input metadata across three engines."""
    inputs = {}
    order = None
    elapsed = {}
    hits = {}
    for engine in ("tlc", "before", candidate):
        durations = []
        hit_trials = []
        for run in range(1, runs + 1):
            batch = rows(root / f"{engine}-{suite}-{run}.jsonl")
            if len(batch) != {"loop": 100, "scenarios": 11}[suite]:
                raise ValueError(f"incomplete {engine}/{suite}/{run}")
            sequence = []
            measurements = []
            hit_indexes = []
            prior = {}
            for i, row in enumerate(batch):
                digest = row["content"]
                metadata = {k: row[k] for k in ("model", "outcome", "distinct")}
                if row["index"] != i or inputs.setdefault(digest, metadata) != metadata:
                    raise ValueError(f"input, count or verdict mismatch at {engine}/{suite}/{run}/{i}")
                sequence.append([row["label"], digest])
                if not math.isfinite(row["ms"]) or row["ms"] < 0:
                    raise ValueError("invalid duration")
                measurements.append(row["ms"])
                if engine != "tlc":
                    if row["cache_hit"]:
                        if engine != candidate or prior.get(digest) != row["evidence"] or not row["key"]:
                            raise ValueError("hit without matching original evidence")
                        hit_indexes.append(i)
                    prior[digest] = row["evidence"]
            if order is None:
                order = sequence
            if order != sequence:
                raise ValueError(f"unpaired {engine}/{suite}/{run}")
            durations.append(measurements)
            hit_trials.append(hit_indexes)
        elapsed[engine] = durations
        if engine != "tlc":
            hits[engine] = hit_trials
    return {"inputs": inputs, "order": order, "elapsed_ms": elapsed, "cache_hit_indexes": hits}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("samples", type=Path, help="directory containing driver JSONL files")
    parser.add_argument("--candidate", default="after", help="candidate file prefix")
    parser.add_argument("--runs", type=int, default=3)
    args = parser.parse_args()
    if args.runs < 1:
        parser.error("at least one paired run is required")
    root = args.samples
    report = {
        "schema": 1,
        "timer": "foreground client wall time, milliseconds",
        "host": json.loads((root / "host.json").read_text()),
        "identities": json.loads((root / "identities.json").read_text()),
        "scenarios": series(root, "scenarios", args.candidate, args.runs),
        "loop": series(root, "loop", args.candidate, args.runs),
    }
    for suite in ("prepare", "audit", "signed-restore"):
        report[suite] = [
            rows(path) for path in sorted(root.glob(f"{args.candidate}-{suite}-*.jsonl"))
        ]
    print(json.dumps(report, sort_keys=True, separators=(",", ":"), allow_nan=False))


if __name__ == "__main__":
    main()
