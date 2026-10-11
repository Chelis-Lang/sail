#!/usr/bin/env python3
"""One unchanged repair fixture with bounded, public-only timing retention."""

import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import re
import runpy
import stat
import sys
import tempfile

FIXTURE_SHA = "9880f200f99007e048727dbe8348eb93f9e53937df601eb16f2d9fdc9ceaebd7"
MAX_SCAN = 4096
MAX_FILES = 257  # One RPC export and at most 256 setup files.
MAX_TOTAL = 4 * 1024 * 1024
JOB = re.compile(r"[0-9]{1,20}-[0-9]{1,10}-[0-9]{1,20}\Z")
SETUP = re.compile(r"events\.setup-[0-9]{1,10}-[0-9]{1,2}\.jsonl\Z")
PHASES = set("metadata runtime-context runtime-binaries helper-identity toolchain-probes "
             "sysroot-guard sysroot-handoff sysroot-hash sysroot-barrier sysroot-fill-hint "
             "context-configuration owned-paths initial-snapshot input-guards monitored-snapshot "
             "input-barrier target-cleanup invocation-seal result-publication receipt-publication "
             "compiler-family-publication".split())
NUMERIC = set("sequence started_us dispatch_delay_us total_us read_us lookup_us output_seal_us "
              "freshness_us frame_key_us frame_lookup_us encode_us write_us child_wait_us "
              "child_exit_observed_us document_read_us freshness_establish_us status_save_us "
              "frame_bytes".split())
ROW_KEYS = NUMERIC | {"operation", "frame_hit", "currentness", "write_succeeded"}


def sha_file(path):
    digest = hashlib.sha256()
    with open(path, "rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def unchanged(path, expected):
    try:
        return sha_file(path) == expected
    except OSError:
        return False


def uint(value, bits=64):
    return type(value) is int and 0 <= value < (1 << bits)


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("duplicate-key")
        result[key] = value
    return result


def decode(data):
    def invalid_constant(_):
        raise ValueError("nonfinite-json")
    return json.loads(data, object_pairs_hook=unique_object,
                      parse_constant=invalid_constant)


def validate(data, rpc):
    if rpc:
        value = decode(data)
        assert isinstance(value, dict) and set(value) == {
            "kind", "daemon_pid", "origin_unix_ms", "capacity", "started", "dropped",
            "overwritten", "rows"}, "rpc-schema"
        assert value["kind"] == "diagnostic-job-rpc-stages" and uint(value["capacity"]) and value["capacity"] == 256
        assert uint(value["daemon_pid"], 32)
        assert all(uint(value[k]) for k in ("origin_unix_ms", "started", "dropped", "overwritten"))
        assert isinstance(value["rows"], list) and len(value["rows"]) <= 256
        for row in value["rows"]:
            assert isinstance(row, dict) and set(row) == ROW_KEYS, "rpc-row-schema"
            assert all(row[k] is None or uint(row[k]) for k in NUMERIC)
            assert all(uint(row[k]) for k in ("sequence", "started_us", "dispatch_delay_us", "total_us"))
            assert row["operation"] in ("result", "status", "worker_publication", "other")
            assert row["currentness"] in (None, "current", "historical", "unknown")
            assert row["frame_hit"] is None or type(row["frame_hit"]) is bool
            assert type(row["write_succeeded"]) is bool
        return len(value["rows"])
    lines = data.splitlines()
    assert len(lines) <= 64, "setup-rows"
    for line in lines:
        row = decode(line)
        assert isinstance(row, dict) and set(row) == {
            "kind", "phase", "pid", "started_ms", "elapsed_ms"}, "setup-schema"
        assert row["kind"] == "managed-setup-phase" and isinstance(row["phase"], str) and row["phase"] in PHASES
        assert uint(row["pid"], 32)
        for key in ("started_ms", "elapsed_ms"):
            number = row[key]
            assert type(number) in (int, float) and math.isfinite(number) and number >= 0
    return len(lines)


def open_directory(name, parent=None):
    return os.open(name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC,
                   dir_fd=parent)


def read_regular(parent, name, limit, consumed):
    fd = os.open(name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC,
                 dir_fd=parent)
    try:
        before = os.fstat(fd)
        assert stat.S_ISREG(before.st_mode) and before.st_size <= limit, "file-kind-or-size"
        with os.fdopen(fd, "rb", closefd=False) as stream:
            data = stream.read(limit + 1)
        consumed(len(data))
        after = os.fstat(fd)
        identity = lambda s: (s.st_dev, s.st_ino, s.st_size, s.st_mtime_ns, s.st_ctime_ns)
        named = os.stat(name, dir_fd=parent, follow_symlinks=False)
        assert len(data) == before.st_size <= limit
        assert identity(before) == identity(after) == identity(named), "file-changed"
        return data
    finally:
        os.close(fd)


def retain_timings(root, destination):
    """Read only fixed public names through no-follow directory descriptors."""
    result = {"files": [], "rejected": [], "scan_entries": 0, "bytes": 0,
              "inspected_bytes": 0, "setup_files_seen": 0,
              "partial": False, "rpc_present": False, "rpc_coverage_complete": False,
              "boundary": "after original fixture finally; before original TemporaryDirectory cleanup"}
    fds = []
    try:
        for part in (str(root), "cache", "projects", "jobs"):
            fds.append(open_directory(part, fds[-1] if fds else None))
        jobs = fds[-1]
        destination.mkdir(mode=0o700)

        def capture(parent, name, member, rpc=False):
            if not rpc:
                result["setup_files_seen"] += 1
                if result["setup_files_seen"] > 256:
                    result["partial"] = True
                    return
            if len(result["files"]) >= MAX_FILES:
                result["partial"] = True
                return
            try:
                remaining = MAX_TOTAL - result["inspected_bytes"]
                if not remaining:
                    result["partial"] = True
                    return
                def consumed(length):
                    result["inspected_bytes"] += length
                # Include the growth sentinel even when identity or schema later refuses.
                data = read_regular(parent, name, min(remaining - 1, 256 * 1024 if rpc else 16 * 1024), consumed)
                rows = validate(data, rpc)
                if result["bytes"] + len(data) > MAX_TOTAL:
                    result["partial"] = True
                    return
                target = destination / member
                target.parent.mkdir(mode=0o700, exist_ok=True)
                with target.open("xb") as output:
                    os.chmod(target, 0o600)
                    output.write(data)
                assert target.read_bytes() == data, "copy-mismatch"
                result["bytes"] += len(data)
                result["files"].append({"member": member, "bytes": len(data),
                                        "sha256": hashlib.sha256(data).hexdigest(), "rows": rows})
                result["rpc_present"] |= rpc
                if rpc:
                    value = decode(data)
                    complete = (value["dropped"] == value["overwritten"] == 0
                                and value["started"] == len(value["rows"]))
                    result["rpc_coverage_complete"] = complete
                    result["rpc_coverage"] = {key: value[key] for key in ("started", "dropped", "overwritten")}
                    result["partial"] |= not complete
            except (OSError, ValueError, AssertionError, OverflowError) as error:
                result["partial"] = True
                if len(result["rejected"]) < MAX_FILES:
                    result["rejected"].append({"member": member, "reason": type(error).__name__})

        # A missing shutdown export is an unavailable observation, not an empty successful trace.
        capture(jobs, "rpc-timings.json", "rpc-timings.json", rpc=True)
        with os.scandir(jobs) as entries:
            for entry in entries:
                result["scan_entries"] += 1
                if result["scan_entries"] > MAX_SCAN:
                    result["partial"] = True
                    break
                if not JOB.fullmatch(entry.name):
                    continue
                try:
                    job = open_directory(entry.name, jobs)
                except OSError:
                    result["partial"] = True
                    continue
                try:
                    with os.scandir(job) as files:
                        for file in files:
                            result["scan_entries"] += 1
                            if result["scan_entries"] > MAX_SCAN:
                                result["partial"] = True
                                break
                            if SETUP.fullmatch(file.name):
                                capture(job, file.name, entry.name + "/" + file.name)
                finally:
                    os.close(job)
                if result["scan_entries"] > MAX_SCAN:
                    break
    except (OSError, ValueError, AssertionError) as error:
        result["partial"] = True
        result["unavailable"] = type(error).__name__
    finally:
        for fd in reversed(fds):
            os.close(fd)
    return result


def temporary_factory(original, destination, captures):
    class CapturedTemporary:
        def __init__(self, *args, **kwargs):
            self.original = original(*args, **kwargs)
        def __enter__(self):
            return self.original.__enter__()
        def __exit__(self, kind, value, traceback):
            try:
                captures.append(retain_timings(Path(self.original.name), destination))
            except Exception as error:
                captures.append({"partial": True, "unavailable": type(error).__name__, "files": []})
            finally:
                # Retention can neither suppress an original exception nor skip original cleanup.
                self.original.__exit__(kind, value, traceback)
            return False
    def factory(*args, **kwargs):
        if not args and kwargs == {"prefix": "sr-", "dir": "/tmp"}:
            return CapturedTemporary(*args, **kwargs)
        return original(*args, **kwargs)
    return factory


def run(args):
    if sys.flags.optimize:
        raise RuntimeError("original fixture assertions require ordinary Python mode")
    assert sha_file(args.fixture) == FIXTURE_SHA, "original fixture digest mismatch"
    assert sha_file(args.sail) == args.sail_sha256, "product digest mismatch"
    fixture = runpy.run_path(args.fixture, run_name="original_repair_fixture")
    output = Path(args.out)
    output.mkdir(mode=0o700)
    captures = []
    original = tempfile.TemporaryDirectory
    prior = os.environ.get("SAIL_JOB_RPC_TIMINGS")
    passed = False
    tempfile.TemporaryDirectory = temporary_factory(original, output / "timings", captures)
    os.environ["SAIL_JOB_RPC_TIMINGS"] = "1"
    try:
        fixture["run"](argparse.Namespace(sail=args.sail, engine_build=args.engine_build,
                                         verus_source=args.verus_source, out=str(output / "public"),
                                         timeout=120, environment_label=args.environment_label +
                                         "; timing-only diagnostic; no acceptance credit"))
        passed = True
    finally:
        tempfile.TemporaryDirectory = original
        if prior is None:
            os.environ.pop("SAIL_JOB_RPC_TIMINGS", None)
        else:
            os.environ["SAIL_JOB_RPC_TIMINGS"] = prior
        active_error = sys.exc_info()[0] is not None
        record = {"kind": "original-repair-publication-timing-diagnostic", "original_fixture_passed": passed,
                  "qualification_credit": False, "fixture_sha256": FIXTURE_SHA,
                  "product_sha256": args.sail_sha256, "capture": captures,
                  "original_fixture_unchanged_after": unchanged(args.fixture, FIXTURE_SHA),
                  "product_unchanged_after": unchanged(args.sail, args.sail_sha256),
                  "limits": {"scan_entries": MAX_SCAN, "files": MAX_FILES, "aggregate_bytes": MAX_TOTAL,
                             "setup_bytes": 16384, "setup_rows": 64, "rpc_bytes": 262144, "rpc_rows": 256},
                  "orphan_absence_certified": False, "original_limits": [120, 30, 5]}
        try:
            (output / "diagnostic.json").write_text(json.dumps(record, indent=2) + "\n")
        except OSError:
            if not active_error:
                raise
            print("Timing diagnostic retention failed; original fixture failure preserved", file=sys.stderr)
    return 0 if (passed and captures and all(not r["partial"] for r in captures)
                 and record["original_fixture_unchanged_after"] and record["product_unchanged_after"]) else 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for flag in ("fixture", "sail", "sail-sha256", "engine-build", "verus-source", "out", "environment-label"):
        parser.add_argument("--" + flag, required=True)
    return run(parser.parse_args())


if __name__ == "__main__":
    sys.exit(main())
