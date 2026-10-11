#!/usr/bin/env python3
"""Bounded offline whole-graph components; never a proof or reader acceptance."""

import argparse
import concurrent.futures
import hashlib
import importlib.util
import json
import math
import pathlib
import socket
import struct
import threading
import time


def digest(data):
    return hashlib.sha256(data).hexdigest()


def profile(frame, producer, collector, samples):
    """Keep complete graphs and original exact()/json.loads; all owners join."""
    expected = json.loads(frame[4:])
    body = frame[4:]
    producer_bytes = producer.read_bytes()
    expected_producer = digest(producer_bytes)

    def run(stage):
        sockets = None
        writer = None
        writer_error = []
        writer_times = []
        try:
            if stage.startswith("transfer"):
                sockets = socket.socketpair()
                for stream in sockets:
                    stream.settimeout(collector.DEADLINE)

                def send():
                    started = time.monotonic_ns()
                    cpu = time.thread_time_ns()
                    try:
                        sockets[1].sendall(frame)
                    except Exception as error:
                        writer_error.append(repr(error))
                    finally:
                        writer_times.append({
                            "wall_ms": (time.monotonic_ns() - started) / 1e6,
                            "thread_cpu_ms": (time.thread_time_ns() - cpu) / 1e6,
                        })

                writer = threading.Thread(target=send, name="owned-result-replay")
            started = time.monotonic_ns()
            cpu = time.thread_time_ns()
            if stage == "decode":
                result = json.loads(body)
            elif stage == "utf8":
                result = body.decode("utf-8")
            elif stage == "read_hash":
                result = digest(producer.read_bytes())
            elif stage == "read":
                result = producer.read_bytes()
            elif stage == "hash":
                result = digest(producer_bytes)
            elif stage.startswith("transfer"):
                writer.start()
                size = struct.unpack(">I", collector.exact(sockets[0], 4))[0]
                if size != len(body) or size > collector.MAX_FRAME:
                    raise ValueError("replay frame length changed")
                read_started = time.monotonic_ns()
                data = collector.exact(sockets[0], size)
                read_done = time.monotonic_ns()
                result = json.loads(data) if stage == "transfer_decode" else data
            else:
                raise ValueError("unknown component")
            done = time.monotonic_ns()
            sample = {
                "wall_ms": (done - started) / 1e6,
                "thread_cpu_ms": (time.thread_time_ns() - cpu) / 1e6,
            }
            if stage.startswith("transfer"):
                sample.update(read_ms=(read_done-read_started)/1e6,
                              decode_ms=(done-read_done)/1e6)
            # Full equality is outside the component timer, including both
            # independent full reports and every dependency/function field.
            if stage in ("decode", "transfer_decode"):
                if result != expected:
                    raise ValueError("complete graph changed")
            elif stage == "transfer":
                if result != body:
                    raise ValueError("complete body changed")
            elif stage == "utf8":
                if result.encode("utf-8") != body:
                    raise ValueError("UTF-8 round trip changed")
            elif stage == "read":
                if result != producer_bytes:
                    raise ValueError("complete producer bytes changed")
            elif result != expected_producer:
                raise ValueError("sealed producer changed")
            return sample
        finally:
            if sockets:
                for stream in sockets:
                    stream.close()
            if writer is not None and writer.ident is not None:
                writer.join(collector.DEADLINE + 1)
                if writer.is_alive():
                    raise RuntimeError("owned replay writer did not close")
                if writer_error:
                    raise RuntimeError("owned replay writer failed: " + repr(writer_error))
                if writer_times and "sample" in locals():
                    sample["writer"] = writer_times[0]

    results = {}
    for workers in (1, 4):
        for stage in ("decode", "utf8", "read", "hash", "read_hash", "transfer", "transfer_decode"):
            measured = []
            with concurrent.futures.ThreadPoolExecutor(max_workers=workers) as pool:
                # Complete one wave before scheduling the next. Validation is
                # outside timers and cannot overlap the following wave.
                for offset in range(0, samples, workers):
                    futures = [pool.submit(run, stage)
                               for _ in range(min(workers, samples-offset))]
                    measured.extend(future.result() for future in futures)
            ordered = sorted(item["wall_ms"] for item in measured)
            results[stage + "_" + str(workers)] = {
                "median_ms": (ordered[(len(ordered)-1)//2]+ordered[len(ordered)//2])/2,
                "p95_ms": ordered[math.ceil(len(ordered)*.95)-1],
                "samples": measured,
            }
    return results


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--public-result", required=True, type=pathlib.Path)
    parser.add_argument("--producer", required=True, type=pathlib.Path)
    parser.add_argument("--collector", required=True, type=pathlib.Path)
    parser.add_argument("--out", required=True, type=pathlib.Path)
    parser.add_argument("--samples", type=int, default=32)
    args = parser.parse_args()
    if not 4 <= args.samples <= 64 or args.samples % 4:
        parser.error("samples must be a multiple of four between 4 and 64")
    for path in (args.public_result, args.producer, args.collector):
        if not path.is_file() or path.stat().st_size > 32 * 1024 * 1024:
            parser.error("input is not a bounded regular file")
    public = json.loads(args.public_result.read_bytes())
    body = json.dumps({"version": 1, "body": public},
                      ensure_ascii=False, separators=(",", ":")).encode()
    if len(body) != 4312245:
        raise ValueError("reconstruction differs from observed complete body length")
    envelope = json.loads(body)
    if envelope["body"] != public or public["response"] != "result":
        raise ValueError("full public Result reconstruction changed")
    if public["value"]["result"] != public["value"]["repair"]["batch"]["full"]:
        raise ValueError("independent full report copies disagree")
    spec = importlib.util.spec_from_file_location("original_collector", args.collector)
    collector = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(collector)
    if digest(args.collector.read_bytes()) != "3d5dfa5d47bbf7377c2d6c79c8ae9618eae6c4a080d66d31bb858fbb8e611720":
        raise ValueError("original Python collector identity changed")
    frame = struct.pack(">I", len(body)) + body
    args.out.mkdir(exist_ok=False)
    (args.out / "reconstructed.frame").write_bytes(frame)
    record = {
        "kind": "offline_reconstructed_complete_result_components",
        "live_acceptance": False, "captured_wire": False,
        "member_order": "sorted saved public graph, not original wire order",
        "freshness_or_currentness_validation_timed": False,
        "original_collector_changed": False,
        "full_graph_equality_checked": True,
        "body_bytes": len(body), "body_sha256": digest(body),
        "producer_bytes": args.producer.stat().st_size,
        "producer_sha256": digest(args.producer.read_bytes()),
        "script_sha256": digest(pathlib.Path(__file__).read_bytes()),
        "collector_sha256": digest(args.collector.read_bytes()),
        "samples_per_case": args.samples,
        "results": profile(frame, args.producer, collector, args.samples),
    }
    (args.out / "components.json").write_text(json.dumps(record, indent=2) + "\n")


if __name__ == "__main__":
    main()
