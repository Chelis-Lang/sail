#!/usr/bin/env python3
"""Run installed SlateDB WAL maintenance checks on the pinned two-writer configurations."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
from time import monotonic_ns

SOURCE = "37f80bb106ca1e5c9981d0a91f51de4fdf04bfcbb9b6fcadfb8e7b16b463821c"
CONFIG = "03d666d0af4def6b6e9b7ded231f7dc8d5f114ddb03ec46f45254dce70233169"
LIBRARIES = {
    "FiniteSetsExt.tla": "dc5d6944395dd18e2f8acf696296808a9460cbbf1b9888f88c5ac4c83a5237f9",
    "Folds.tla": "aa59063fd600bb640b2ae24dc85ef770277ef5bf7955092b76b8b471790086da",
    "Functions.tla": "b54ff63b7c76c327525c17c188d5f9f5e53d92f3fd701f5e2ba54f0f54391063",
}
INVARIANTS = ["TypeOK", "UniqueEpochs", "ValidGarbageCollectors", "ValidWriters",
              "ConsistentMachineData", "ManifestRepresentsCommittedLog"]
COUNTS = {
    "Medium": [(74637, 198123, 69), (52260, 138979, 63)],
    "TwoRetry": [(3761377, 12508420, 93), (1601824, 5211591, 84)],
}
VERSIONS = [0, 1, 1, 0]
LIMIT_MS = 300000


def digest(data):
    return hashlib.sha256(data).hexdigest()


def pinned(path, expected):
    data = path.read_bytes()
    if digest(data) != expected:
        raise ValueError(f"unexpected input bytes at {path}")
    return data.decode()


def invoke(sail, work, args, document):
    payload = json.dumps(document)
    clock = work / "rss.txt"
    command = ["/usr/bin/time", "-f", "%M", "-o", str(clock), str(sail), "runtime", *args, "--file", "-"]
    start = monotonic_ns()
    completed = subprocess.run(command, input=payload, capture_output=True, text=True, timeout=LIMIT_MS/1000 + 30)
    elapsed = monotonic_ns() - start
    if completed.returncode:
        raise RuntimeError(f"{args} exited {completed.returncode}: {completed.stderr[:2048]}")
    return json.loads(completed.stdout), {"wall_ns": elapsed, "peak_rss_kib": int(clock.read_text())}


def response_counts(response, expected):
    assert response["outcome"] == "holds" and response["completion"] == "safety", response["outcome"]
    assert response["requested_checks"] == {"invariants": INVARIANTS, "properties": [],
                                             "possible": [], "postconditions": [], "deadlock": False}
    stats = response["native_report"]["statistics"]
    assert stats["left_on_queue"] == 0 and stats["initial_generated"] == 1
    assert tuple(stats[k] for k in ("distinct", "generated", "depth")) == expected
    return {k: stats[k] for k in ("initial_generated", "generated", "distinct", "depth", "left_on_queue")}


def run(args, work):
    cfg = pinned(args.source / "SlateDBWAL.cfg", CONFIG).replace("Writers = {w1, w2, w3}", "Writers = {w1, w2}")
    if args.case == "Medium":
        # Retain byte identity with Dive's archived Medium.cfg, including its comment.
        cfg = cfg.replace("MaxWriterRestarts = 1", "MaxWriterRestarts = 0")
    files = {name: pinned(args.modules / name, sha) for name, sha in LIBRARIES.items()}
    files["SlateDBWAL.tla"] = pinned(args.source / "SlateDBWAL.tla", SOURCE)
    assert cfg.count("EnableFix = TRUE") == 1
    configs = [cfg, cfg.replace("EnableFix = TRUE", "EnableFix = FALSE")]
    requests = [{"version": 1, "model": {"root": "SlateDBWAL.tla", "config": "Case.cfg",
                 "files": {**files, "Case.cfg": c}},
                 "operation": {"kind": "check", "max_states": 5000000, "max_depth": 1000}} for c in configs]
    rows = []
    receipts = []
    for tag, version in zip(("cold", "edit", "repeat", "undo", "fresh-fixed", "fresh-edited"), VERSIONS + [0, 1]):
        cache = work / (tag if tag.startswith("fresh-") else "history")
        envelope = {"version": 1, "input": {"kind": "temporal", "job": {"obligation_id": "wal-source-maintenance",
                    "requirement_ids": [], "wall_time_ms": LIMIT_MS, "request": requests[version]}}}
        output, row = invoke(args.sail, work, ["execute", "--cache", str(cache)], envelope)
        result = output["output"]["result"]
        response = result["response"]
        identity = response["request_sha256"]
        assert identity == result["request_sha256"]
        counts = response_counts(response, COUNTS[args.case][version])
        receipt = digest(json.dumps(response, sort_keys=True, separators=(",", ":")).encode())
        if tag in ("cold", "edit"):
            receipts.append(receipt)
        hit = tag in ("repeat", "undo")
        assert result["cache_hit"] == hit and (not hit or receipt == receipts[version])
        rows.append(dict(row, tag=tag, request_sha256=identity, response_sha256=receipt,
                         cache_hit=hit, counts=counts, worker_sha256=result["worker_sha256"]))
    batch = {"version": 1, "wall_time_ms": LIMIT_MS, "checks": [requests[v] for v in VERSIONS]}
    output, timing = invoke(args.sail, work, ["edit-session"], batch)
    assert output["outcome"] == "completed" and len(output["entries"]) == len(VERSIONS)
    session = []
    for index, (version, entry) in enumerate(zip(VERSIONS, output["entries"])):
        evidence = entry["evidence"]
        assert evidence["route"] == "fresh" and entry["request_sha256"] == rows[index]["request_sha256"]
        counts = response_counts(evidence["response"], COUNTS[args.case][version])
        session.append(dict(route=evidence["route"], elapsed_us=entry["elapsed_us"], counts=counts,
                            request_sha256=entry["request_sha256"]))
    return dict(case=args.case, module_sha256=SOURCE, author_cfg_sha256=CONFIG,
                cfg_sha256=[digest(c.encode()) for c in configs], library_sha256=LIBRARIES,
                wall_limit_ms=LIMIT_MS, max_states=5000000, max_depth=1000, invariants=INVARIANTS,
                backend_source=output["backend_source"], worker_sha256=output["worker_sha256"], ordinary=rows,
                session=dict(timing, entries=session))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sail", type=Path, required=True)
    parser.add_argument("--source", type=Path, required=True, help="upstream slatedb directory")
    parser.add_argument("--modules", type=Path, required=True, help="pinned CommunityModules modules directory")
    parser.add_argument("--case", choices=COUNTS, required=True)
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="wal-edits-") as work:
        print(json.dumps(run(args, Path(work)), indent=2))


if __name__ == "__main__":
    main()
