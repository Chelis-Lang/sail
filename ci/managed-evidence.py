"""Private timeout evidence for the owned managed fixture, never proof authority."""

import hashlib
import json
import os
from pathlib import Path
import stat
import time

MAX_BYTES = 1024 * 1024
MAX_PROCESSES = 128
MAX_ENTRIES = 512
MAX_TASKS = 128
MAX_FDS = 64


class Capture:
    def __init__(self):
        self.remaining = MAX_BYTES
        self.deadline = time.monotonic() + .5
        self.errors = []
        self.limited = False

    def available(self):
        available = self.remaining > 0 and time.monotonic() < self.deadline
        self.limited |= not available
        return available

    def read(self, path, limit=65536):
        if not self.available():
            return None
        try:
            descriptor = os.open(path, os.O_RDONLY | os.O_CLOEXEC | os.O_NOFOLLOW | os.O_NONBLOCK)
            try:
                if not stat.S_ISREG(os.fstat(descriptor).st_mode):
                    self.errors.append({"operation": "read", "file": str(path),
                                        "error": "not a regular diagnostic file"})
                    return None
                with os.fdopen(descriptor, "rb", closefd=False) as source:
                    result = source.read(min(limit, self.remaining))
            finally:
                os.close(descriptor)
            self.remaining -= len(result)
            return result
        except OSError as error:
            self.errors.append({"operation": "read", "file": str(path),
                                "error": type(error).__name__})
            return None

    def link(self, path):
        if not self.available():
            return None
        try:
            value = os.readlink(path)
            size = len(os.fsencode(value))
            if size > self.remaining:
                self.limited = True
                return None
            self.remaining -= size
            return value
        except OSError:
            return None


def process_stat(raw):
    """Decode the kernel comm boundary rather than splitting comm whitespace."""
    if raw is None:
        return None
    try:
        end = raw.rindex(b")")
        fields = raw[end + 2:].split()
        return {"state": fields[0].decode("ascii"), "parent": int(fields[1]),
                "process_group": int(fields[2]), "user_ticks": int(fields[11]),
                "system_ticks": int(fields[12]), "start_ticks": int(fields[19]),
                "resident_pages": int(fields[21])}
    except (ValueError, IndexError, UnicodeError):
        return None


def compiler_selection(raw):
    """Retain only crate-selector facts, not command environments or full argv."""
    if raw is None:
        return None
    arguments = raw.split(b"\0")
    arguments = arguments[:arguments.index(b"--")] if b"--" in arguments else arguments
    name = None
    for index, argument in enumerate(arguments):
        if argument == b"--crate-name" and index + 1 < len(arguments):
            name = arguments[index + 1].decode("utf-8", "replace")
        elif argument.startswith(b"--crate-name="):
            name = argument[len(b"--crate-name="):].decode("utf-8", "replace")
    return {"crate_name": name, "literal_verus_crate": b"--verus-crate" in arguments,
            "literal_is_vstd": b"--is-vstd" in arguments}


def owned_processes(root_pid, capture, proc=Path("/proc"), expected_start_ticks=None):
    """Follow all owned task-child links, binding queued PID birth identities.

    An external observer can bind the root to its previously recorded birth
    ticks. Missing or changed identities are partial observation, not evidence
    that no child exists, and never permission to signal a discovered process.
    """
    pending = [(root_pid, expected_start_ticks)]
    seen = set()
    result = []
    while pending and len(seen) < MAX_PROCESSES and capture.available():
        pid, expected = pending.pop(0)
        if pid in seen:
            continue
        seen.add(pid)
        directory = proc / str(pid)
        before = process_stat(capture.read(directory / "stat"))
        if before is None:
            continue
        if expected is not None and before["start_ticks"] != expected:
            capture.limited = True
            capture.errors.append({"operation": "process-identity", "pid": pid,
                                   "error": "birth identity changed before observation"})
            continue
        document = {"pid": pid, **before}
        document["compiler_selection"] = compiler_selection(capture.read(directory / "cmdline"))
        children = set()
        try:
            with os.scandir(directory / "task") as tasks:
                for index, task in enumerate(tasks):
                    if index >= MAX_TASKS or not capture.available():
                        capture.limited = True
                        break
                    raw = capture.read(Path(task.path) / "children", 4096)
                    if raw:
                        children.update(int(value) for value in raw.split() if value.isdigit())
        except OSError:
            pass
        for name in ["comm", "wchan"]:
            raw = capture.read(directory / name, 4096)
            document[name] = None if raw is None else raw.decode("utf-8", "replace").strip()
        for name in ["cwd", "exe"]:
            document[name] = capture.link(directory / name)
        fds = []
        try:
            with os.scandir(directory / "fd") as entries:
                for index, entry in enumerate(entries):
                    if index >= MAX_FDS or not capture.available():
                        capture.limited = True
                        break
                    target = capture.link(entry.path)
                    if target is not None:
                        fds.append({"fd": entry.name, "target": target})
        except OSError:
            pass
        after = process_stat(capture.read(directory / "stat"))
        document["identity_stable"] = (after is not None
                                       and before["start_ticks"] == after["start_ticks"]
                                       and before["parent"] == after["parent"])
        document["fds"] = fds
        result.append(document)
        if document["identity_stable"]:
            # Bind each child to this same parent before following it. A stale
            # PID or reparented process is observation loss, never owned cleanup.
            for child in sorted(children):
                child_stat = process_stat(capture.read(proc / str(child) / "stat"))
                if child_stat and child_stat["parent"] == pid:
                    if len(pending) + len(seen) < MAX_PROCESSES:
                        pending.append((child, child_stat["start_ticks"]))
                    else:
                        capture.limited = True
    capture.limited |= bool(pending)
    return result


def scratch_inventory(cache, capture):
    """Inventory only run sidecars; omit Cargo artifacts and environment records."""
    root = cache / "projects"
    pending = [root]
    result = []
    seen_entries = 0
    while pending and seen_entries < MAX_ENTRIES and capture.available():
        directory = pending.pop(0)
        try:
            if directory.is_symlink():
                continue
            with os.scandir(directory) as entries:
                for entry in entries:
                    seen_entries += 1
                    if seen_entries > MAX_ENTRIES or not capture.available():
                        capture.limited = True
                        break
                    path = Path(entry.path)
                    relative = path.relative_to(cache)
                    parts = relative.parts
                    # Before runs, enumerate only project/context directories.
                    if len(parts) <= 3:
                        if entry.is_dir(follow_symlinks=False):
                            pending.append(path)
                        continue
                    if parts[3] != "runs":
                        continue
                    if entry.is_dir(follow_symlinks=False):
                        pending.append(path)
                    elif entry.is_file(follow_symlinks=False):
                        metadata = entry.stat(follow_symlinks=False)
                        result.append({"relative_path": str(relative), "bytes": metadata.st_size})
        except OSError as error:
            capture.errors.append({"operation": "inventory", "error": type(error).__name__})
    capture.limited |= bool(pending)
    return result


def capture_timeout(pid, cache, events, destination):
    started = time.monotonic()
    capture = Capture()
    raw = capture.read(events, 262144)
    document = {"scope": "timeout-only private observation before owned process-group kill",
                "acceptance_timeout_seconds": 240,
                "capture_is_proof_authority": False,
                "processes": owned_processes(pid, capture),
                "scratch_inventory": scratch_inventory(cache, capture)}
    document["progress"] = None if raw is None else {
        "bytes_captured": len(raw), "sha256": hashlib.sha256(raw).hexdigest(),
        "records": raw.decode("utf-8", "replace").splitlines()}
    try:
        document["clock_ticks_per_second"] = os.sysconf("SC_CLK_TCK")
    except (OSError, ValueError):
        document["clock_ticks_per_second"] = None
    document.update(capture_seconds=time.monotonic() - started,
                    bytes_read=MAX_BYTES - capture.remaining,
                    limited=capture.limited, errors=capture.errors,
                    work_bounds={"bytes": MAX_BYTES, "processes": MAX_PROCESSES,
                                 "entries": MAX_ENTRIES, "tasks_per_process": MAX_TASKS,
                                 "fds_per_process": MAX_FDS,
                                 "scheduling_budget_seconds": .5},
                    limitation="Kernel reads cannot be interrupted by an elapsed-time check; all observations may be partial and do not establish a timeout cause.")
    destination.write_text(json.dumps(document, indent=2) + "\n")
