#!/usr/bin/env python3
"""Collect the paired S7 JSONL samples from perf-temporal-reuse.py into citable JSON."""
import argparse
import hashlib
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


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def compare_with_record(root, baseline_path, candidate, runs):
    """Verify a new candidate against the published TLC and native control observations."""
    baseline_bytes = baseline_path.read_bytes()
    baseline = json.loads(baseline_bytes)
    record = {
        "schema": 1,
        "comparison": {"dataset": baseline_path.name, "sha256": sha256(baseline_bytes),
                       "engines": ["pinned TLC", "Sail 0.3.0", "original exact temporal receipts"]},
        "driver_sha256": sha256(Path(__file__).with_name("perf-temporal-reuse.py").read_bytes()),
        "suites": {}, "checked_rows": 0,
    }
    for suite in ("scenarios", "loop"):
        expected = baseline[suite]
        evidence, identities, trials = [], {}, []
        for trial in range(runs):
            raw = (root / f"{candidate}-{suite}-{trial + 1}.jsonl").read_bytes()
            batch = [json.loads(line) for line in raw.splitlines()]
            hits, indexes, result_rows, prior, times = [], [], [], {}, []
            for i, (row, (label, digest)) in enumerate(zip(batch, expected["order"], strict=True)):
                known = expected["inputs"][digest]
                if (row["index"], row["label"], row["content"]) != (i, label, digest) or any(
                    row[key] != known[key] for key in ("model", "outcome", "distinct")
                ):
                    raise ValueError(f"input, verdict or count changed at {suite}/{trial + 1}/{i}")
                if not math.isfinite(row["ms"]) or row["ms"] < 0:
                    raise ValueError("invalid foreground duration")
                times.append(row["ms"])
                if row["cache_hit"]:
                    if not row["key"] or prior.get(digest) != row["evidence"]:
                        raise ValueError(f"missing original evidence for replay at {suite}/{i}")
                    hits.append(i)
                prior[digest] = row["evidence"]
                identity = (digest, row["request_sha256"], row["evidence"], row["key"])
                if identity not in identities:
                    identities[identity] = len(evidence)
                    evidence.append(dict(zip(("input", "request_sha256", "evidence", "cache_key"), identity, strict=True)))
                indexes.append(identities[identity])
                result_rows.append(json.dumps([i, digest, row["outcome"], row["distinct"]], separators=(",", ":")))
            if hits != expected["cache_hit_indexes"]["after"][trial]:
                raise ValueError(f"replay positions changed at {suite}/{trial + 1}")
            trials.append({"raw_jsonl_sha256": sha256(raw), "elapsed_ms": times, "cache_hit_indexes": hits,
                           "evidence_indexes": indexes,
                           "result_rows_sha256": sha256(("\n".join(result_rows) + "\n").encode())})
            record["checked_rows"] += len(batch)
        record["suites"][suite] = {"evidence": evidence, "trials": trials}
    return record


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("samples", type=Path, help="directory containing driver JSONL files")
    parser.add_argument("--candidate", default="after", help="candidate file prefix")
    parser.add_argument("--runs", type=int, default=3)
    parser.add_argument("--compare", type=Path, help="check new samples against a published three-engine S7 JSON record")
    args = parser.parse_args()
    if args.runs < 1:
        parser.error("at least one paired run is required")
    root = args.samples
    if args.compare:
        report = compare_with_record(root, args.compare, args.candidate, args.runs)
        print(json.dumps(report, separators=(",", ":"), allow_nan=False))
        return
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
