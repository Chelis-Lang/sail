#!/usr/bin/env python3
"""Read-only interpretation of bounded, matched HTTP owner observations."""
import argparse
import json
from pathlib import Path
import re


def stat_identity(raw):
    if not isinstance(raw, str):
        raise ValueError("task stat unavailable")
    name, fields = raw.rsplit(")", 1)
    return int(name.split("(", 1)[0].strip()), int(fields.split()[19])


def sample_threads(sample):
    if not sample["complete"]:
        raise ValueError("bounded traversal incomplete")
    threads = {}
    rows = sample["rows"]
    for index, (label, stat, sched, _wait) in enumerate(rows):
        if label == "runtime-post-stat":
            continue
        identity = stat_identity(stat)
        cpu, queued, _slices = map(int, sched.split())
        if label.startswith("reqwest-runtime:"):
            following = rows[index + 1]
            if following[0] != "runtime-post-stat" or stat_identity(following[1]) != identity:
                raise ValueError("runtime birth changed during sample")
        if identity in threads:
            raise ValueError("duplicate observed task identity")
        threads[identity] = {"label": label, "cpu_ns": cpu, "runqueue_ns": queued}
    if not any(value["label"] == "calling-thread" for value in threads.values()):
        raise ValueError("calling thread absent")
    if not any(value["label"].startswith("reqwest-runtime:") for value in threads.values()):
        raise ValueError("I/O runtime absent")
    return threads


def interval(before, after):
    try:
        first, last = sample_threads(before), sample_threads(after)
        if first.keys() != last.keys():
            raise ValueError("live task set changed across observations")
        rows = []
        for identity, start in first.items():
            end = last[identity]
            cpu = end["cpu_ns"] - start["cpu_ns"]
            queued = end["runqueue_ns"] - start["runqueue_ns"]
            if min(cpu, queued) < 0:
                raise ValueError("counter regressed")
            rows.append({"identity": list(identity), "label": start["label"],
                         "cpu_ms": cpu / 1000000, "runqueue_ms": queued / 1000000})
        return {"status": "available", "sample_start_wall_ms": (after["start_us"] - before["start_us"]) / 1000,
                "threads": rows, "total_cpu_ms": sum(row["cpu_ms"] for row in rows)}
    except (KeyError, TypeError, ValueError, IndexError) as error:
        return {"status": "unavailable", "reason": str(error)}


def arm(root, name):
    raw = (root / (name + "-matched-lookup.stderr")).read_bytes()
    if len(raw) > 32 * 1024 * 1024:
        return {"status": "unavailable", "reason": "raw interpretation bound exceeded"}
    text = raw.decode("utf-8")
    samples = {}
    for line in text.splitlines():
        marker = "SAIL_HTTP_OWNER "
        if marker not in line:
            continue
        data = line.split(marker, 1)[1]
        if len(data.encode()) > 2 * 1024 * 1024:
            return {"status": "unavailable", "reason": "sample interpretation bound exceeded"}
        value = json.loads(data)
        label = value["label"]
        if label in samples:
            return {"status": "unavailable", "reason": "duplicate phase label"}
        samples[label] = value
    observations = {}
    for label, before, after in [("send", "before-send", "after-send"),
                                 ("body", "before-body", "after-body"),
                                 ("whole_transport", "before-send", "after-body"),
                                 ("helper_to_publication", "helper-start", "published")]:
        if before in samples and after in samples:
            observations[label] = interval(samples[before], samples[after])
        else:
            observations[label] = {"status": "unavailable", "reason": "phase not observed"}
    phases = re.findall(r'\("(lookup-start|published)", ([0-9]+)\)', text)
    start = [int(value) for label, value in phases if label == "lookup-start"]
    published = [int(value) for label, value in phases if label == "published"]
    observations["whole_lookup_to_publication_ms"] = ((max(published) - min(start)) / 1000
                                                       if start and published else None)
    observers = re.findall(r'\((true|false), ([0-9.]+)(ns|µs|ms|s)\)', text)
    scale = {"ns": 0.000001, "µs": 0.001, "ms": 1, "s": 1000}
    observations["caller_observations"] = [{"hit": hit == "true", "elapsed_ms": float(value) * scale[unit]}
                                            for hit, value, unit in observers]
    observations["coalesced_followers"] = [int(value) for value in re.findall(r"coalesced_followers=([0-9]+)", text)]
    observations["sample_cost_us"] = {label: value["end_us"] - value["start_us"] for label, value in samples.items()}
    return observations


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", type=Path)
    args = parser.parse_args()
    report = {"scope": "one matched synchronous observation, not performance qualification",
              "arms": {}, "limitations": ["metadata perturbs timing", "counter reads are non-atomic",
                                            "missing/incomplete/birth-changed counters remain unavailable",
                                            "no historical failure attribution or Current authority"]}
    for name in ("baseline", "candidate"):
        try:
            report["arms"][name] = arm(args.root, name)
        except (OSError, UnicodeError, ValueError, KeyError, TypeError, IndexError) as error:
            report["arms"][name] = {"status": "unavailable", "reason": str(error)}
    (args.root / "comparison.json").write_text(json.dumps(report, indent=2) + "\n")


if __name__ == "__main__":
    main()
