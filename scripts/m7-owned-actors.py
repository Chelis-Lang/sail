#!/usr/bin/env python3
"""Owned-actor helpers shared by the M7 reader and standard-conformance controllers.

These are the process-ownership pieces of the retired Python repair runner. The
repair benchmark itself is `sail-bench repair` (docs/bench/m7-repair-acceptance.md).
Uses only Python's standard library.
"""

import hashlib
import importlib.util
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time

REPO = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location("m7_job_protocol", REPO / "scripts/m7-job-read-bench.py")
protocol = importlib.util.module_from_spec(spec)
spec.loader.exec_module(protocol)

def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def write_json(path, value):
    payload = json.dumps(value, indent=2, sort_keys=True) + "\n"
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(mode="w", encoding="utf-8", dir=path.parent,
                                         prefix="." + path.name + ".", delete=False) as output:
            temporary = Path(output.name)
            output.write(payload)
            output.flush()
            os.fsync(output.fileno())
        os.replace(temporary, path)
        directory = os.open(path.parent, os.O_RDONLY)
        try:
            os.fsync(directory)
        finally:
            os.close(directory)
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)


def signal_owned(process, signal_number):
    if process.poll() is None:
        try:
            os.killpg(process.pid, signal_number)
        except ProcessLookupError:
            pass


def interrupt_owned(process):
    """Give the CLI's interrupt handler time to cancel its separate daemon job."""
    signal_owned(process, signal.SIGINT)
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        signal_owned(process, signal.SIGKILL)
        process.wait(timeout=5)


class OwnedDaemons:
    """Private workload daemons, never a discovered or pre-existing service."""
    def __init__(self, output):
        self.output = output
        self.actors = {}
        self.records = []

    def __enter__(self):
        return self

    def start(self, mode, binary, workspace, environment):
        address = Path(environment["SAIL_DAEMON_SOCKET"])
        identity = (str(binary), str(address), environment["SAIL_CACHE_DIR"])
        if mode in self.actors:
            if self.actors[mode][3] != identity or self.actors[mode][0].poll() is not None:
                raise ValueError("SAIL0941: owned daemon identity changed or daemon exited")
            return
        if address.exists() or address.is_symlink():
            raise ValueError("SAIL0941: refusing a pre-existing workload daemon socket")
        stdout = (self.output / ("daemon-" + mode + ".stdout")).open("wb")
        try:
            stderr = (self.output / ("daemon-" + mode + ".stderr")).open("wb")
        except BaseException:
            stdout.close()
            raise
        try:
            actor = subprocess.Popen([str(binary), "serve", "--cache", environment["SAIL_CACHE_DIR"],
                                      "--socket", str(address), "--idle-timeout", "0"],
                                     cwd=workspace, env=environment, stdout=stdout, stderr=stderr,
                                     start_new_session=True)
        except BaseException:
            stdout.close()
            stderr.close()
            raise
        self.actors[mode] = (actor, stdout, stderr, identity)
        self.records.append({"mode":mode,"pid":actor.pid,"binary_sha256":sha(binary),"started":True})
        write_json(self.output / "actor-lifecycle.json", {"actors":self.records,"cleanup_errors":[]})
        deadline = time.monotonic() + 5
        while not address.exists():
            if actor.poll() is not None or time.monotonic() >= deadline:
                raise ValueError("SAIL0941: private workload daemon did not become ready")
            time.sleep(.01)

    def __exit__(self, failure_type, failure, traceback):
        errors = []
        for mode, (actor, stdout, stderr, _) in reversed(list(self.actors.items())):
            started = time.monotonic()
            try:
                # SIGTERM runs Sail's owned-job shutdown before the cache server exits.
                signal_owned(actor, signal.SIGTERM)
                try:
                    actor.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    signal_owned(actor, signal.SIGKILL)
                    actor.wait(timeout=10)
                self.records.append({"mode":mode,"pid":actor.pid,"stopped":True,
                                     "exit_code":actor.returncode,"elapsed_s":time.monotonic()-started})
            except (OSError, subprocess.SubprocessError) as error:
                errors.append(f"{mode} daemon cleanup: {error}")
            finally:
                stdout.close()
                stderr.close()
        try:
            write_json(self.output / "actor-lifecycle.json", {"actors":self.records,"cleanup_errors":errors})
        except OSError as error:
            errors.append(f"daemon lifecycle evidence: {error}")
        if errors:
            print("SAIL0941: " + "; ".join(errors), file=sys.stderr)
            if failure_type is None:
                raise ValueError("SAIL0941: private workload daemon cleanup failed")
        return False


def warm_repair_state(doc, allow_setup):
    """Only an explicit first-run declaration transition permits one retry."""
    batch = doc.get("batch", {})
    full = batch.get("full", {})
    if not batch.get("batched") or not batch.get("manifest_sha256") or batch.get("focus") is None:
        raise ValueError("SAIL0942: repair warmup did not use the real source engine")
    if doc.get("selection_error") or full.get("error") or full.get("exit_code") != 0:
        raise ValueError("SAIL0942: original repair warmup did not verify")
    targets = [target for target in full.get("targets", []) if target.get("requested")]
    if len(targets) != 1 or not targets[0].get("complete"):
        raise ValueError("SAIL0942: original repair warmup has incomplete full evidence")
    if doc.get("accepted") is True and full.get("currentness") == "current":
        return "ready"
    if (allow_setup and doc.get("accepted") is False
            and full.get("currentness") in ("unknown", "historical")
            and "Build-script input declarations were established during this run" in full.get("notes", [])):
        return "declarations-established"
    raise ValueError("SAIL0942: repair warmup lacks Current accepted full evidence")

