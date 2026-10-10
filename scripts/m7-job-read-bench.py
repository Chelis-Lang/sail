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


def proof_crate(command):
    """Retain only the exact crate selector, never arbitrary compiler arguments."""
    names = []
    for index, value in enumerate(command):
        if value == b"--":
            break
        if value == b"--crate-name":
            if index + 1 >= len(command):
                return None
            names.append(command[index + 1])
        elif value.startswith(b"--crate-name="):
            names.append(value.split(b"=", 1)[1])
    if len(names) != 1:
        return None
    try:
        name = names[0].decode("ascii")
    except UnicodeError:
        return None
    return name if name and name.isidentifier() else None


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
            candidate = {
                "pid": int(directory.name), "parent": int(fields[1]),
                "name": raw[raw.index("(") + 1:closing], "state": fields[0],
                "started_ticks": int(fields[19]),
            }
            if candidate["name"] == "rust_verify":
                with (directory / "cmdline").open("rb") as command:
                    arguments = command.read(65537)
                after = (directory / "stat").read_text()
                end = after.rindex(")")
                check = after[end + 2:].split()
                if (len(arguments) > 65536 or int(check[19]) != candidate["started_ticks"]
                        or int(check[1]) != candidate["parent"] or check[0] not in ("R", "S", "D")
                        or after[after.index("(") + 1:end] != "rust_verify"):
                    # CPU counters and R/S state may change during real work;
                    # PID identity, ancestry and exact executable name may not.
                    continue
                candidate["crate_name"] = proof_crate(arguments.split(b"\0"))
            result[int(directory.name)] = candidate
        except (OSError, ValueError, IndexError):
            # A process may exit during enumeration; an absent verifier cannot
            # satisfy the acceptance contract.
            continue
    return result


def active_verifiers(table, worker, started_ticks, crate):
    """Only live Rust verifiers descended from the exact worker qualify."""
    owner = table.get(worker)
    if owner is None or owner["started_ticks"] != started_ticks or owner["state"] not in ("R", "S", "D"):
        return []
    verified = []
    for candidate in table.values():
        if (candidate["name"] != "rust_verify" or candidate["state"] not in ("R", "S", "D")
                or candidate.get("crate_name") != crate):
            continue
        parent = candidate["parent"]
        child_started = candidate["started_ticks"]
        seen = set()
        while parent in table and parent not in seen:
            if table[parent]["started_ticks"] > child_started:
                break
            if parent == worker:
                verified.append(candidate)
                break
            seen.add(parent)
            child_started = table[parent]["started_ticks"]
            parent = table[parent]["parent"]
    return sorted(verified, key=lambda item: item["pid"])


def continuing_verifier(before, after):
    """One immutable compiler identity must span the timed RPC."""
    def identity(process):
        return (process["pid"], process["started_ticks"], process["parent"], process["crate_name"])
    return bool({identity(process) for process in before} & {identity(process) for process in after})


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
    if (evidence["id"] != job or evidence["state"] != "completed"
            or evidence["currentness"] != "current" or evidence["input_revision"] != revision):
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
        verifiers_before = active_verifiers(process_table(), args.active_worker, worker_started, args.active_crate)
        operation = "status" if index % 2 == 0 else "result"
        response, elapsed, length = call(address, {"operation": operation, "id": args.job})
        verifiers_after = active_verifiers(process_table(), args.active_worker, worker_started, args.active_crate)
        active, _, _ = call(address, {"operation": "status", "id": args.active_job})
        return {"operation": operation, "wall_ms": elapsed, "response_bytes": length,
                "current": valid_established(response, args.job, revision),
                "active_before": active_before, "active_after": status(active)["state"],
                "verifier_continued": continuing_verifier(verifiers_before, verifiers_after),
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
              and sample["verifier_continued"]
              and sample["active_before"] == sample["active_after"] == "running" for sample in samples)
              and all(value["p95_ms"] < 200 for value in summary.values()))
    result = {"schema_version": 1, "platform": platform.platform(),
              "execution_environment": args.environment, "clients": args.clients,
              "deadline_seconds": DEADLINE, "job": args.job, "active_job": args.active_job,
              "active_worker": worker,
              "active_crate": args.active_crate,
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
    parser.add_argument("--active-crate", required=True,
                        help="Exact Cargo library crate being verified, excluding ordinary dependency compilation")
    parser.add_argument("--clients", type=int, default=4)
    parser.add_argument("--samples", type=int, default=200)
    parser.add_argument("--establish-timeout", type=float, default=600)
    parser.add_argument("--environment", required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    if (os.environ.get("CI") or not 2 <= args.clients <= 16 or args.samples < 100
            or proof_crate([b"--crate-name", args.active_crate.encode()]) != args.active_crate):
        raise ValueError("SAIL0943: local acceptance requires 2–16 clients and at least 100 samples")
    return measure(args)


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (ValueError, KeyError, OSError) as error:
        raise SystemExit("error[SAIL0943]: " + str(error))
