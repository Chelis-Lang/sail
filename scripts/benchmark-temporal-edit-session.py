#!/usr/bin/env python3
"""Compare complete installed edit sessions to fresh native Sail checks on the same inputs."""
import argparse
from copy import deepcopy
from hashlib import sha256
import json
from pathlib import Path
import subprocess
import tempfile
from time import monotonic_ns


def request(tla, cfg, states):
    model = {"root": "Star.tla", "config": "Star.cfg", "files": {"Star.tla": tla, "Star.cfg": cfg}}
    return {"version": 1, "model": model, "operation": {"kind": "check", "max_states": states, "max_depth": 100}}


def edit(base, old, new):
    current = deepcopy(base)
    files = current["model"]["files"]
    root = current["model"]["root"]
    assert files[root].count(old) == 1
    files[root] = files[root].replace(old, new)
    return current


def cases(width, fixtures):
    cfg = "INIT Init\nNEXT Next\nINVARIANT Inv\n"
    source = f"""---- MODULE Star ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == IF x = 0 THEN x' \\in 1..{width} ELSE IF x = {width} THEN x' = {width + 1} ELSE x' = x
Inv == x <= {width + 1}
====
"""
    holds = request(source, cfg, width + 3)
    bad = edit(holds, "Inv == x <=", "Inv == x <")
    yield "completed-safety", [holds, edit(holds, "Inv == x <=", "Inv == 0 <= x /\\ x <="), bad], ["holds", "holds", "refuted"]
    yield "refuted-prefix", [bad, edit(bad, "Inv == x <", "Inv == x /=")], ["refuted", "refuted"]
    tw = min(width, 20000)
    temporal = request(f"""---- MODULE Star ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == IF x = 0 THEN x' \\in 1..{tw} ELSE x' = x
Spec == Init /\\ [][Next]_x
P == <>(x >= 0)
Q == <>(x = 0)
====
""", "SPECIFICATION Spec\nPROPERTY P Q\n", tw + 2)
    yield "completed-temporal", [temporal, edit(temporal, "P == <>(x >= 0)", "P == <>(x <= 0)"),
                                 edit(temporal, "P == <>(x >= 0)", f"P == <>(x > {tw})")], ["holds", "holds", "refuted"]
    if fixtures:
        diehard = json.loads((fixtures / "DieHard/envelope.json").read_text())["input"]["job"]["request"]
        yield "s7-diehard", [diehard, edit(diehard, "NotSolved == big # 4", "NotSolved == big # 4 /\\ small <= 3"),
                             edit(diehard, "NotSolved == big # 4", "NotSolved == big # 6")], ["refuted", "refuted", "holds"]
        for label, name, old, new in [
            ("s7-mutex", "MCLamportMutex", "TypeOK BoundedNetwork Mutex", "TypeOK Mutex"),
            ("s7-internal-memory", "MCLiveInternalMemory", "\nPROPERTY Liveness\n", "\n"),
        ]:
            original = json.loads((fixtures / name / "envelope.json").read_text())["input"]["job"]["request"]
            modified = deepcopy(original)
            key = modified["model"]["config"]
            files = modified["model"]["files"]
            assert files[key].count(old) == 1
            files[key] = files[key].replace(old, new)
            yield label, [original, modified], ["holds", "holds"]


def call(sail, args, value):
    start = monotonic_ns()
    process = subprocess.run([sail, "runtime", *args, "--file", "-"], input=json.dumps(value),
                             text=True, capture_output=True, timeout=330, check=True)
    wall_us = (monotonic_ns() - start) // 1000
    return json.loads(process.stdout), wall_us


def compare(sail, root, name, checks, outcomes, sample):
    batch = {"version": 1, "wall_time_ms": 300000, "checks": checks}

    def ordinary():
        rows = []
        for i, check in enumerate(checks):
            job = {"version": 1, "input": {"kind": "temporal", "job": {"obligation_id": name,
                   "requirement_ids": [], "wall_time_ms": 300000, "request": check}}}
            reply, us = call(sail, ["execute", "--cache", str(root / f"{name}-{sample}-{i}")], job)
            result = reply["output"]["result"]
            assert not result["cache_hit"] and result["response"]["outcome"] == outcomes[i]
            rows.append((result["response"], us))
        return rows

    if sample % 2:
        fresh = ordinary()
    session, wall = call(sail, ["edit-session"], batch)
    if not sample % 2:
        fresh = ordinary()
    assert session["outcome"] == "completed" and len(session["entries"]) == len(checks)
    rows = []
    for entry, (native, fresh_us), expected in zip(session["entries"], fresh, outcomes, strict=True):
        assert entry["request_sha256"] == native["request_sha256"]
        evidence = entry["evidence"]
        if evidence["route"] in ("fresh", "exact-request"):
            route = evidence["route"]
            if route == "exact-request":
                basis = evidence["basis_entry"]
                assert 0 <= basis < len(rows) and checks[basis] == checks[len(rows)]
                evidence = session["entries"][basis]["evidence"]
                assert evidence["route"] == "fresh"
            result = evidence["response"]
            assert result["outcome"] == expected
            stats = result["native_report"]["statistics"]
            trace = result["itf_traces"]
        elif evidence["route"] == "established-first-temporal-claim":
            assert expected == "holds" and native["requested_checks"]["properties"] == [evidence["property"]]
            assert evidence["basis_sha256"] == fresh[0][0]["request_sha256"]
            stats = evidence["basis_counts"]
            trace = []
            route = evidence["route"]
        else:
            decision = evidence["decision"]
            assert decision["outcome"] == expected
            stats = evidence["decision_counts"]
            trace = []
            if expected == "refuted":
                assert decision["validation"]["outcome"] == "valid"
                trace = [decision["trace"]]
            route = evidence["structure"] + ("-recheck" if evidence["reused"] else "-capture")
        keys = ("initial_generated", "generated", "distinct", "depth", "left_on_queue")
        counts = {k: stats[k] for k in keys}
        assert counts == {k: native["native_report"]["statistics"][k] for k in keys}
        assert trace == native["itf_traces"], (name, route, "witness differs")
        rows.append({"route": route, "outcome": expected, "request_sha256": entry["request_sha256"],
                     "counts": counts, "worker_us": entry["elapsed_us"], "fresh_process_us": fresh_us})
    if name in ("completed-safety", "refuted-prefix", "completed-temporal"):
        assert all(row["route"].endswith("-recheck") for row in rows[1:]), rows
    if name == "s7-internal-memory":
        assert [r["route"] for r in rows] == ["fresh", "established-first-temporal-claim"], rows
    return {"session_process_us": wall, "backend_source": session["backend_source"],
            "worker_sha256": session["worker_sha256"], "rows": rows}


def durable_projection(sail, root, case, original, edited, trials):
    safety = case == "s7-mutex"
    route = "established-invariant-subset" if safety else "established-first-temporal-claim"
    requirement = "mutual-exclusion" if safety else "first-liveness-property"
    cache = root / f"durable-{case}-cache"

    def envelope(check, label):
        return {"version": 1, "input": {"kind": "temporal", "job": {
            "obligation_id": label, "requirement_ids": [requirement],
            "wall_time_ms": 300000, "request": check}}}

    prev = envelope(original, "prior")
    previous_file = root / f"{case}-previous.json"
    previous_file.write_text(json.dumps(prev))
    old, seed_us = call(sail, ["execute", "--cache", str(cache)], prev)
    basis = old["output"]["result"]
    assert basis["response"]["outcome"] == "holds" and not basis["cache_hit"]
    samples = []
    for n in range(trials):
        current = envelope(deepcopy(edited), f"edit-{n}")
        files = current["input"]["job"]["request"]["model"]["files"]
        files[edited["model"]["config"]] += f"\n\n\\* sampled edit {n}\n"
        result, us = call(sail, ["recheck", "--previous", str(previous_file), "--cache", str(cache)], current)
        assert result["route"] == route and result["outcome"] == "holds", result
        assert result["obligation_id"] == f"edit-{n}" and result["requirement_ids"] == [requirement]
        assert result["previous_request_sha256"] == basis["request_sha256"]
        assert result["worker_sha256"] == basis["worker_sha256"] and result["previous_receipt_key"]
        ordinary, fresh_us = call(sail, ["execute", "--cache", str(cache)], current)
        fresh = ordinary["output"]["result"]
        assert not fresh["cache_hit"] and fresh["response"]["outcome"] == "holds"
        assert result["request_sha256"] == fresh["request_sha256"]
        inventory = fresh["response"]["requested_checks"]
        if safety:
            assert result["invariants"] == inventory["invariants"]
        else:
            assert [result["property"]] == inventory["properties"] == ["LivenessProperty"]
            assert all(basis["response"]["native_report"]["statistics"][k] == v for k, v in result["basis_counts"].items())
        assert basis["response"]["native_report"]["statistics"]["distinct"] == fresh["response"]["native_report"]["statistics"]["distinct"] == (724274 if safety else 4408)
        samples.append({"projected_process_us": us, "fresh_process_us": fresh_us,
                        "current_sha256": result["request_sha256"]})
    return {"establishing_process_us": seed_us, "original_sha256": basis["request_sha256"],
            "stats": basis["response"]["native_report"]["statistics"], "samples": samples}

def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("sail", type=Path)
    p.add_argument("--width", type=int, default=200000)
    p.add_argument("--samples", type=int, default=3)
    p.add_argument("--s7-fixtures", type=Path, help="directory from perf-temporal-reuse.py fixtures")
    args = p.parse_args()
    if args.width < 2 or args.samples < 1:
        p.error("width must be at least 2 and sample count must be positive")
    sail = args.sail.resolve()
    data = {"schema": "sail-temporal-edit-session-v1", "sail_sha256": sha256(sail.read_bytes()).hexdigest(),
            "width": args.width, "samples": args.samples, "cases": {}}
    with tempfile.TemporaryDirectory() as d:
        for name, checks, expected in cases(args.width, args.s7_fixtures):
            data["cases"][name] = [compare(sail, Path(d), name, checks, expected, i) for i in range(args.samples)]
            if name in ("s7-mutex", "s7-internal-memory"):
                data["persisted_" + name.replace("-", "_")] = durable_projection(sail, Path(d), name, *checks, args.samples)
    print(json.dumps(data, indent=2))


if __name__ == "__main__":
    main()
