#!/usr/bin/env python3
"""Bounded read-only Linux metadata observer for one frozen common-target ELF.

This does not run tests, send signals, read argv/environment, or open process
FD targets. Snapshots are partial observations, never closure/cause evidence.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import sys
import time

LIMITS = {
    "scanned_processes": 4096, "selected_processes": 128,
    "retained_identities": 1024, "threads_per_process": 256,
    "threads_total": 1024, "fds_per_process": 256, "fds_total": 2048,
    "read_bytes": 4 * 1024 * 1024, "cycle_seconds": 1.0,
    "record_bytes": 4 * 1024 * 1024, "output_bytes": 32 * 1024 * 1024,
    "polls": 180, "notes": 64,
}
FD_FIELDS = {"pos", "flags", "mnt_id", "ino", "eventfd-count", "pipe-size"}
DEAD = {"Z", "X", "x"}


def parse_stat(data, expected_pid):
    """Parse kernel stat without treating spaces/parentheses in comm as fields."""
    left = data.find(b"(")
    right = data.rfind(b")")
    if left < 1 or right <= left or int(data[:left].strip()) != expected_pid:
        raise ValueError("stat identity")
    fields = data[right + 1:].split()
    if len(fields) < 20 or len(fields[0]) != 1:
        raise ValueError("stat fields")
    values = [int(fields[index]) for index in (1, 11, 12, 19)]
    if any(value < 0 for value in values):
        raise ValueError("negative stat fields")
    return {"pid": expected_pid, "ppid": values[0], "start_ticks": values[3],
            "state": fields[0].decode("ascii"), "utime_ticks": values[1],
            "stime_ticks": values[2]}


def token(row):
    return (row["pid"], row["start_ticks"])


def same_birth(left, right, parent=False):
    return (token(left) == token(right)
            and (not parent or (left["ppid"] == right["ppid"]
                                and right["state"] not in DEAD)))


def descendents(rows, anchors, limit):
    """Require a live birth-ordered chain; recycled intermediate PIDs refuse."""
    selected = {}
    for pid, value in sorted(anchors.items()):
        if len(selected) == limit:
            break
        if (pid in rows and token(rows[pid]) == value["identity"]
                and rows[pid]["state"] not in DEAD):
            selected[pid] = value
    changed = True
    while changed and len(selected) < limit:
        changed = False
        for pid in sorted(rows):
            if len(selected) >= limit:
                break
            child = rows[pid]
            parent = rows.get(child["ppid"])
            if (pid in selected or parent is None or parent["pid"] not in selected
                    or parent["state"] in DEAD or child["state"] in DEAD
                    or child["start_ticks"] < parent["start_ticks"]):
                continue
            selected[pid] = {"identity": token(child),
                             "root": selected[parent["pid"]]["root"],
                             "parent_identity": token(parent)}
            changed = True
    return selected


def fd_metadata(data):
    """Keep only fixed kernel metadata, never arbitrary fdinfo content."""
    result = {}
    for line in data.splitlines():
        key, separator, value = line.partition(b":")
        name = key.decode("ascii", errors="replace")
        value = value.strip()
        if separator and name in FD_FIELDS and len(value) <= 64:
            text = value.decode("ascii", errors="replace")
            if re.fullmatch(r"[0-9a-fA-FxX]+", text):
                result[name] = text
    return result


class Partial(Exception):
    """A fixed bound or unavailable kernel metadata prevented a complete read."""


class Cycle:
    def __init__(self, proc, limits=None):
        self.proc = Path(proc)
        self.limits = LIMITS if limits is None else limits
        self.began = time.monotonic()
        self.bytes = 0
        self.notes = []
        self.omitted_notes = 0
        self.counts = {"threads": 0, "fds": 0}

    def note(self, reason, **fields):
        if len(self.notes) < self.limits["notes"]:
            self.notes.append({"reason": reason, **fields})
        else:
            self.omitted_notes += 1

    def check(self):
        if time.monotonic() - self.began > self.limits["cycle_seconds"]:
            raise Partial("cycle-work-limit")

    def read(self, path, limit):
        self.check()
        if self.bytes + limit + 1 > self.limits["read_bytes"]:
            raise Partial("cycle-byte-limit")
        with Path(path).open("rb") as stream:
            value = stream.read(limit + 1)
        self.bytes += len(value)
        if len(value) > limit:
            raise Partial("metadata-size-limit")
        return value

    def link(self, path):
        self.check()
        value = os.readlink(path)
        size = len(os.fsencode(value))
        self.bytes += size
        if size > 4096 or self.bytes > self.limits["read_bytes"]:
            raise Partial("link-size-limit")
        return value

    def numeric(self, path, limit):
        result = []
        with os.scandir(path) as entries:
            for entry in entries:
                self.check()
                if not entry.name.isascii() or not entry.name.isdecimal():
                    continue
                if len(result) == limit:
                    self.note("directory-count-limit", limit=limit)
                    break
                result.append(int(entry.name))
        return sorted(result)

    def process(self, pid):
        return parse_stat(self.read(self.proc / str(pid) / "stat", 4096), pid)


def unavailable(cycle, error, **fields):
    reason = str(error) if isinstance(error, Partial) else type(error).__name__
    cycle.note(reason, errno=getattr(error, "errno", None), **fields)


def still_chain(cycle, pid, selected, scanned):
    """Revalidate each newly admitted edge against this cycle's birth snapshot."""
    visited = set()
    while True:
        if pid in visited:
            return False
        visited.add(pid)
        before = scanned[pid]
        if not same_birth(before, cycle.process(pid), parent=True):
            return False
        edge = selected[pid].get("parent_identity")
        if edge is None:
            return True  # Either a current ELF root or a previously bound child.
        parent_pid, parent_start = edge
        if parent_pid not in selected or scanned[parent_pid]["start_ticks"] != parent_start:
            return False
        pid = parent_pid


def details(cycle, row):
    pid = row["pid"]
    base = cycle.proc / str(pid)
    result = {**row, "threads": [], "fds": []}
    try:
        result["exe"] = cycle.link(base / "exe")
    except (OSError, Partial) as error:
        unavailable(cycle, error, pid=pid, part="exe")
    try:
        tids = cycle.numeric(base / "task", cycle.limits["threads_per_process"])
        for tid in tids:
            if cycle.counts["threads"] >= cycle.limits["threads_total"]:
                cycle.note("thread-total-limit", pid=pid)
                break
            cycle.counts["threads"] += 1
            task = base / "task" / str(tid)
            try:
                before = parse_stat(cycle.read(task / "stat", 4096), tid)
                comm = cycle.read(task / "comm", 64).decode("utf-8", errors="replace").rstrip("\n")
                wchan = cycle.read(task / "wchan", 128).decode("ascii", errors="replace").rstrip("\n")
                after = parse_stat(cycle.read(task / "stat", 4096), tid)
                if not same_birth(before, after):
                    cycle.note("thread-recycled", pid=pid, tid=tid)
                    continue
                result["threads"].append({**before, "comm": comm, "wchan": wchan})
            except (OSError, ValueError, Partial) as error:
                unavailable(cycle, error, pid=pid, tid=tid, part="thread")
    except (OSError, Partial) as error:
        unavailable(cycle, error, pid=pid, part="tasks")
    try:
        fds = cycle.numeric(base / "fd", cycle.limits["fds_per_process"])
        for fd in fds:
            if cycle.counts["fds"] >= cycle.limits["fds_total"]:
                cycle.note("fd-total-limit", pid=pid)
                break
            cycle.counts["fds"] += 1
            try:
                target = cycle.link(base / "fd" / str(fd))
                info = cycle.read(base / "fdinfo" / str(fd), 4096)
                after_info = cycle.read(base / "fdinfo" / str(fd), 4096)
                metadata = fd_metadata(info)
                after_metadata = fd_metadata(after_info)
                if (cycle.link(base / "fd" / str(fd)) != target
                        or any(metadata.get(key) != after_metadata.get(key) for key in ("ino", "mnt_id", "flags"))):
                    cycle.note("fd-target-changed", pid=pid, fd=fd)
                    continue
                result["fds"].append({"fd": fd, "target": target, "metadata": metadata})
            except (OSError, Partial) as error:
                unavailable(cycle, error, pid=pid, fd=fd, part="fd")
    except (OSError, Partial) as error:
        unavailable(cycle, error, pid=pid, part="fds")
    if not same_birth(row, cycle.process(pid)):
        raise Partial("process-recycled-during-snapshot")
    return result


def elf_identity(path, expected):
    """Refuse private targets, aliases and replacements before any proc census."""
    path = Path(path)
    if not re.fullmatch(r"/work/target/debug/deps/faults-[0-9a-f]+", str(path)):
        raise ValueError("only exact common-target faults ELF allowed")
    if path.is_symlink() or path.resolve() != path:
        raise ValueError("ELF alias refused")
    before = path.stat()
    if (not stat.S_ISREG(before.st_mode) or before.st_size > 128 * 1024 * 1024
            or before.st_uid != os.geteuid()):
        raise ValueError("ELF type/size bound")
    digest = hashlib.sha256()
    total = 0
    with path.open("rb") as stream:
        if stream.read(4) != b"\x7fELF":
            raise ValueError("ELF magic required")
        stream.seek(0)
        while chunk := stream.read(1024 * 1024):
            total += len(chunk)
            if total > 128 * 1024 * 1024:
                raise ValueError("ELF grew beyond bound")
            digest.update(chunk)
    after = path.stat()
    fields = ("st_dev", "st_ino", "st_size", "st_mtime_ns", "st_ctime_ns")
    if any(getattr(before, field) != getattr(after, field) for field in fields):
        raise ValueError("ELF changed while hashing")
    if digest.hexdigest() != expected:
        raise ValueError("ELF digest mismatch")
    return {"path": str(path), "sha256": expected, "bytes": after.st_size,
            "dev": after.st_dev, "ino": after.st_ino, "uid": after.st_uid,
            "mtime_ns": after.st_mtime_ns, "ctime_ns": after.st_ctime_ns}


def elf_unchanged(elf):
    current = Path(elf["path"]).stat()
    return all(getattr(current, field) == elf[key] for field, key in (
        ("st_dev", "dev"), ("st_ino", "ino"), ("st_size", "bytes"),
        ("st_uid", "uid"), ("st_mtime_ns", "mtime_ns"), ("st_ctime_ns", "ctime_ns")))


def is_elf_root(cycle, pid, elf):
    base = cycle.proc / str(pid)
    if cycle.link(base / "exe") != elf["path"]:
        return False
    identity = (base / "exe").stat()
    return identity.st_dev == elf["dev"] and identity.st_ino == elf["ino"]


def snapshot(proc, elf, known, limits=None):
    cycle = Cycle(proc, limits)
    rows, roots, records = {}, {}, []
    try:
        if not elf_unchanged(elf):
            raise Partial("frozen-elf-changed")
        for pid in cycle.numeric(cycle.proc, cycle.limits["scanned_processes"]):
            try:
                rows[pid] = cycle.process(pid)
                if rows[pid]["state"] not in DEAD and is_elf_root(cycle, pid, elf):
                    root = token(rows[pid])
                    roots[pid] = {"identity": root, "root": root}
            except (OSError, ValueError, Partial) as error:
                unavailable(cycle, error, pid=pid, part="discovery")
                if isinstance(error, Partial):
                    break
        anchors = {pid: {"identity": item["identity"], "root": item["root"]}
                   for pid, item in known.items() if pid in rows
                   and token(rows[pid]) == item["identity"]
                   and item["identity"] != item["root"]}
        anchors.update(roots)
        selected = descendents(rows, anchors, cycle.limits["selected_processes"])
        if len(selected) >= cycle.limits["selected_processes"]:
            cycle.note("selected-process-limit")
        for pid, item in sorted(selected.items()):
            try:
                if not still_chain(cycle, pid, selected, rows):
                    cycle.note("ancestry-changed", pid=pid)
                    continue
                if pid in roots and not is_elf_root(cycle, pid, elf):
                    cycle.note("root-executable-changed", pid=pid)
                    continue
                record = details(cycle, rows[pid])
                if not still_chain(cycle, pid, selected, rows):
                    cycle.note("ancestry-changed-after-snapshot", pid=pid)
                    continue
                record["root_identity"] = list(item["root"])
                record["previously_bound"] = pid in known and known[pid]["identity"] == item["identity"]
                records.append(record)
                if pid in known or len(known) < cycle.limits["retained_identities"]:
                    known[pid] = item
                else:
                    cycle.note("retained-identity-limit", pid=pid)
            except (OSError, ValueError, Partial) as error:
                unavailable(cycle, error, pid=pid, part="snapshot")
                if isinstance(error, Partial) and str(error) == "cycle-work-limit":
                    break
        if not elf_unchanged(elf):
            records.clear()
            cycle.note("frozen-elf-changed-after-snapshot")
    except (OSError, Partial) as error:
        unavailable(cycle, error, part="process-directory")
    return {"processes": records, "partial": bool(cycle.notes or cycle.omitted_notes),
            "unavailable": cycle.notes, "omitted_notes": cycle.omitted_notes,
            "scanned_processes": len(rows), "read_bytes": cycle.bytes,
            "work_seconds": time.monotonic() - cycle.began}


class Writer:
    def __init__(self, path, limits=None):
        self.limits = LIMITS if limits is None else limits
        self.stream = open(path, "xb", buffering=0)
        os.chmod(path, 0o600)
        self.bytes = 0

    def write(self, value):
        encoded = bytearray()
        encoder = json.JSONEncoder(ensure_ascii=True, separators=(",", ":"))
        for fragment in encoder.iterencode(value):
            chunk = fragment.encode()
            if len(encoded) + len(chunk) + 1 > self.limits["record_bytes"]:
                return False
            encoded.extend(chunk)
        encoded.extend(b"\n")
        # Reserve a fixed terminal marker even when a snapshot is refused.
        if len(encoded) > self.limits["record_bytes"] or self.bytes + len(encoded) > self.limits["output_bytes"] - 4096:
            return False
        self.write_all(encoded)
        self.bytes += len(encoded)
        return True

    def finish(self, reason):
        encoded = json.dumps({"kind": "terminal", "reason": reason,
                              "observed_bytes": self.bytes, "closure_certified": False}).encode() + b"\n"
        self.write_all(encoded)
        self.stream.close()

    def write_all(self, encoded):
        view = memoryview(encoded)
        while view:
            written = self.stream.write(view)
            if written is None or written <= 0:
                raise OSError("observer output did not progress")
            view = view[written:]


def phase(path):
    if path is None:
        return "unattributed"
    try:
        with Path(path).open("rb") as stream:
            value = stream.read(33)
        return value.decode("ascii").strip() if value in (b"workspace\n", b"solver\n", b"workspace", b"solver") else "unavailable"
    except (OSError, UnicodeError):
        return "unavailable"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--elf", required=True)
    parser.add_argument("--elf-sha256", required=True)
    parser.add_argument("--source", required=True)
    parser.add_argument("--out", required=True)
    parser.add_argument("--stop-file", required=True)
    parser.add_argument("--phase-file")
    parser.add_argument("--interval", type=float, default=5.0)
    args = parser.parse_args()
    if not sys.platform.startswith("linux"):
        raise ValueError("Linux proc observer only")
    if not re.fullmatch(r"[0-9a-f]{64}", args.elf_sha256) or not re.fullmatch(r"[0-9a-f]{40}", args.source):
        raise ValueError("exact digest/source required")
    if not 2 <= args.interval <= 60:
        raise ValueError("interval bound")
    if Path(args.stop_file).exists() or Path(args.out).absolute() == Path(args.stop_file).absolute():
        raise ValueError("fresh distinct stop file required")
    elf = elf_identity(args.elf, args.elf_sha256)
    writer = Writer(args.out)
    began, known, reason = time.monotonic(), {}, "poll-limit"
    script = Path(__file__).read_bytes()
    try:
        writer.write({"kind": "header", "elf": elf, "source": args.source,
                      "source_scope": "caller-provided frozen source label",
                      "observer_sha256": hashlib.sha256(script).hexdigest(),
                      "limits": LIMITS, "interval_seconds": args.interval,
                      "clock_ticks_per_second": os.sysconf("SC_CLK_TCK"),
                      "boot_id": Cycle("/proc").read("/proc/sys/kernel/random/boot_id", 64).decode("ascii").strip(),
                      "acceptance_credit": False, "argv_or_environment_read": False})
        for sequence in range(LIMITS["polls"]):
            if Path(args.stop_file).exists():
                reason = "controller-stop"
                break
            value = snapshot("/proc", elf, known)
            value.update(kind="snapshot", sequence=sequence, monotonic_seconds=time.monotonic() - began,
                         wall_time_ns=time.time_ns(), phase=phase(args.phase_file))
            if not writer.write(value):
                reason = "record-or-output-limit"
                break
            # Short stop-file checks; no signal handlers or process signalling.
            until = time.monotonic() + args.interval
            while time.monotonic() < until and not Path(args.stop_file).exists():
                time.sleep(min(0.2, max(0, until - time.monotonic())))
    except BaseException:
        reason = "observer-error-or-interruption"
        raise
    finally:
        writer.finish(reason)


if __name__ == "__main__":
    main()
