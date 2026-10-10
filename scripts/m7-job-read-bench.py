#!/usr/bin/env python3
"""Measure real job status/result reads while another verification is running.

Local only. Uses the unchanged five-second protocol deadline. Unknown evidence,
an inactive worker, mismatched revisions and incomplete samples fail acceptance.
"""

import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
import os
from pathlib import Path
import platform
import socket
import statistics
import struct
import time

MAX_FRAME = 32 * 1024 * 1024
DEADLINE = 5


def process_table(proc=Path("/proc")):
    """Capture kernel identities without reading process environment variables."""
    result = {}
    for directory in proc.iterdir():
        if not directory.name.isdecimal():
            continue
        try:
            raw = (directory / "stat").read_text()
            closing = raw.rindex(")")
            fields = raw[closing + 2:].split()
            result[int(directory.name)] = {
                "pid": int(directory.name), "parent": int(fields[1]),
                "name": raw[raw.index("(") + 1:closing], "state": fields[0],
                "started_ticks": int(fields[19]),
            }
        except (OSError, ValueError, IndexError):
            # A process may exit during enumeration; an absent verifier cannot
            # satisfy the acceptance contract.
            continue
    return result


def active_verifiers(table, worker, started_ticks):
    """Only live Rust verifiers descended from the exact worker qualify."""
    owner = table.get(worker)
    if owner is None or owner["started_ticks"] != started_ticks or owner["state"] == "Z":
        return []
    verified = []
    for candidate in table.values():
        if candidate["name"] != "rust_verify" or candidate["state"] == "Z":
            continue
        parent = candidate["parent"]
        seen = set()
        while parent in table and parent not in seen:
            if parent == worker:
                verified.append(candidate)
                break
            seen.add(parent)
            parent = table[parent]["parent"]
    return sorted(verified, key=lambda item: item["pid"])


def exact(stream, count):
    result = bytearray()
    while len(result) < count:
        chunk = stream.recv(count - len(result))
        if not chunk:
            raise ValueError("SAIL0943: truncated job response")
        result.extend(chunk)
    return bytes(result)


def call(endpoint, request):
    started = time.monotonic()
    payload = json.dumps({"version": 1, "body": request}).encode()
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as stream:
        stream.settimeout(DEADLINE)
        stream.connect(str(endpoint))
        stream.sendall(struct.pack(">I", len(payload)) + payload)
        length = struct.unpack(">I", exact(stream, 4))[0]
        if length > MAX_FRAME:
            raise ValueError("SAIL0943: oversized job response")
        envelope = json.loads(exact(stream, length))
    elapsed = (time.monotonic() - started) * 1000
    if envelope["version"] != 1 or envelope["body"]["response"] == "error":
        raise ValueError("SAIL0943: job service error: " + repr(envelope))
    return envelope["body"], elapsed, length


def endpoint(cache_socket):
    resolved = cache_socket.resolve()
    suffix = hashlib.sha256(os.fsencode(resolved)).hexdigest()[:8]
    return resolved.with_name("jobs-v1-proof-v1-" + suffix + ".sock")


def status(response):
    if response["response"] == "status":
        return response["value"]
    if response["response"] == "result":
        return response["value"]["job"]
    raise ValueError("SAIL0943: unexpected job response")


def valid_established(response, job, revision):
    evidence = status(response)
    if evidence["id"] != job or evidence["state"] != "completed" or evidence["currentness"] != "current":
        return False
    if response["response"] == "result":
        result = response["value"]["result"]
        return (result is not None and result["currentness"] == "current"
                and result["error"] is None and result["input_revision"] == revision)
    return evidence["input_revision"] == revision


def p95(values):
    ordered = sorted(values)
    return ordered[max(0, (95 * len(ordered) + 99) // 100 - 1)]


def measure(args):
    address = endpoint(args.socket)
    worker = process_table().get(args.active_worker)
    command = (Path("/proc") / str(args.active_worker) / "cmdline").read_bytes().split(b"\0")
    if worker is None or len(command) < 2 or command[1] != b"_repair-worker":
        raise ValueError("SAIL0943: active PID is not a repair verification worker")
    worker_started = worker["started_ticks"]
    # Establishment may hash the runtime once asynchronously. It is outside the
    # latency census, but its actual duration and every incomplete read remain visible.
    started = time.monotonic()
    establishment_reads = 0
    revision = None
    while time.monotonic() - started < args.establish_timeout:
        response, _, _ = call(address, {"operation": "result", "id": args.job})
        establishment_reads += 1
        document = response.get("value", {}).get("result")
        if document is not None:
            revision = document["input_revision"]
            if valid_established(response, args.job, revision):
                break
        time.sleep(0.1)
    else:
        raise ValueError("SAIL0943: completed current evidence never established")
    establishment_ms = (time.monotonic() - started) * 1000

    def sample(index):
        active, _, _ = call(address, {"operation": "status", "id": args.active_job})
        active_before = status(active)["state"]
        verifiers_before = active_verifiers(process_table(), args.active_worker, worker_started)
        operation = "status" if index % 2 == 0 else "result"
        response, elapsed, length = call(address, {"operation": operation, "id": args.job})
        verifiers_after = active_verifiers(process_table(), args.active_worker, worker_started)
        active, _, _ = call(address, {"operation": "status", "id": args.active_job})
        return {"operation": operation, "wall_ms": elapsed, "response_bytes": length,
                "current": valid_established(response, args.job, revision),
                "active_before": active_before, "active_after": status(active)["state"],
                "verifiers_before": verifiers_before, "verifiers_after": verifiers_after}

    with ThreadPoolExecutor(max_workers=args.clients) as readers:
        samples = list(readers.map(sample, range(args.samples)))
    summary = {}
    for operation in ("status", "result"):
        selected = [sample for sample in samples if sample["operation"] == operation]
        values = [sample["wall_ms"] for sample in selected]
        summary[operation] = {"count": len(values), "median_ms": statistics.median(values),
                              "p95_ms": p95(values), "max_ms": max(values)}
    passed = (len(samples) == args.samples and all(sample["current"]
              and sample["verifiers_before"] and sample["verifiers_after"]
              and sample["active_before"] == sample["active_after"] == "running" for sample in samples)
              and all(value["p95_ms"] < 200 for value in summary.values()))
    result = {"schema_version": 1, "platform": platform.platform(),
              "execution_environment": args.environment, "clients": args.clients,
              "deadline_seconds": DEADLINE, "job": args.job, "active_job": args.active_job,
              "active_worker": worker,
              "input_revision": revision, "establishment_ms": establishment_ms,
              "establishment_reads": establishment_reads, "samples": samples,
              "summary": summary, "passed": passed}
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps({"summary": summary, "passed": passed}, indent=2))
    return 0 if passed else 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--socket", type=Path, required=True, help="SMT daemon socket")
    parser.add_argument("--job", required=True, help="Completed real verification job")
    parser.add_argument("--active-job", required=True, help="Another currently running verification")
    parser.add_argument("--active-worker", type=int, required=True,
                        help="Linux PID of that job's _repair-worker; live descendant verifier required")
    parser.add_argument("--clients", type=int, default=4)
    parser.add_argument("--samples", type=int, default=200)
    parser.add_argument("--establish-timeout", type=float, default=600)
    parser.add_argument("--environment", required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    if os.environ.get("CI") or not 2 <= args.clients <= 16 or args.samples < 100:
        raise ValueError("SAIL0943: local acceptance requires 2–16 clients and at least 100 samples")
    return measure(args)


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (ValueError, KeyError, OSError) as error:
        raise SystemExit("error[SAIL0943]: " + str(error))
