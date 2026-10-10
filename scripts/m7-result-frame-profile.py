#!/usr/bin/env python3
"""Offline whole-frame phase diagnostics; never a live-reader acceptance result."""

import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
from pathlib import Path
import platform
import socket
import statistics
import struct
import threading
import time

ERROR = "SAIL0946: "
MAX_FRAME = 32 * 1024 * 1024
DEADLINE = 5


def load_frame(path):
    with path.open("rb") as source:
        frame = source.read(MAX_FRAME + 5)
    if len(frame) < 4:
        raise ValueError(ERROR + "missing frame length")
    size = struct.unpack(">I", frame[:4])[0]
    if size > MAX_FRAME or len(frame) != size + 4:
        raise ValueError(ERROR + "oversized or incomplete frame")
    envelope = json.loads(frame[4:])
    if envelope["version"] != 1:
        raise ValueError(ERROR + "unsupported frame version")
    return frame, envelope


def read_stdout(path):
    with path.open("rb") as source:
        data = source.read(MAX_FRAME + 1)
    if len(data) > MAX_FRAME:
        raise ValueError(ERROR + "oversized worker report")
    return data


def measure(operation, threads, samples):
    def one(_):
        started = time.perf_counter()
        value = operation()
        return (time.perf_counter() - started) * 1000, value

    with ThreadPoolExecutor(max_workers=threads) as workers:
        observed = list(workers.map(one, range(samples)))
    times = sorted(time for time, _ in observed)
    return {
        "samples": samples, "threads": threads,
        "median_ms": statistics.median(times),
        "p95_ms": times[(95 * samples + 99) // 100 - 1], "max_ms": times[-1],
    }, [value for _, value in observed]


def transfer(frame, use_into):
    left, right = socket.socketpair()
    left.settimeout(DEADLINE)
    right.settimeout(DEADLINE)
    failures = []

    def send():
        try:
            left.sendall(frame)
        except OSError as error:
            failures.append(error)

    producer = threading.Thread(target=send)
    producer.start()

    def exact(count):
        if use_into:
            output = bytearray(count)
            view = memoryview(output)
            received = 0
            while received < count:
                amount = right.recv_into(view[received:])
                if not amount:
                    raise ValueError(ERROR + "truncated response")
                received += amount
            return output
        output = bytearray()
        while len(output) < count:
            chunk = right.recv(count - len(output))
            if not chunk:
                raise ValueError(ERROR + "truncated response")
            output.extend(chunk)
        return bytes(output)

    try:
        size = struct.unpack(">I", exact(4))[0]
        if size > MAX_FRAME:
            raise ValueError(ERROR + "oversized response")
        value = json.loads(exact(size))
    finally:
        right.close()
        producer.join(timeout=DEADLINE)
        left.close()
    if producer.is_alive() or failures:
        raise ValueError(ERROR + "owned producer failed to complete")
    return value


def profile(frame, expected, samples=8, stdout=None):
    if samples < 4 or samples % 4:
        raise ValueError(ERROR + "samples must be at least four and divisible by four")
    payload = frame[4:]
    results = {}
    for threads in (1, 4):
        operations = {
            "full_json_decode": lambda: json.loads(payload),
            "current_exact_transfer_full_decode": lambda: transfer(frame, False),
            "experimental_recv_into_transfer_full_decode": lambda: transfer(frame, True),
        }
        for name, operation in operations.items():
            result, decoded = measure(operation, threads, samples)
            if any(value != expected for value in decoded):
                raise ValueError(ERROR + "original complete graph changed")
            results[f"{name}_{threads}"] = result
    result = {
        "kind": "offline_archived_whole_result_phase_profile",
        "platform": platform.platform(), "python": platform.python_version(),
        "payload_bytes": len(payload), "frame_sha256": hashlib.sha256(frame).hexdigest(),
        "complete_graph_equality_checked": True, "live_acceptance": False,
        "socket_timeout_seconds": DEADLINE,
        "socketpair_includes_owned_producer_startup": True,
        "experimental_transport_is_not_a_product_change": True, "results": results,
    }
    if stdout is not None:
        result["sealed_stdout_bytes"] = len(stdout)
        result["sealed_stdout_sha256"] = hashlib.sha256(stdout).hexdigest()
        results["sealed_stdout_sha256"], _ = measure(
            lambda: hashlib.sha256(stdout).digest(), 1, samples)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--frame", type=Path, required=True)
    parser.add_argument("--stdout", type=Path)
    parser.add_argument("--samples", type=int, default=8)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    frame, expected = load_frame(args.frame)
    stdout = read_stdout(args.stdout) if args.stdout else None
    result = profile(frame, expected, args.samples, stdout)
    if args.stdout:
        result["results"]["sealed_stdout_read"], _ = measure(
            lambda: read_stdout(args.stdout), 1, args.samples)
    encoded = json.dumps(result, indent=2) + "\n"
    if args.output:
        args.output.write_text(encoded)
    else:
        print(encoded, end="")


if __name__ == "__main__":
    main()
