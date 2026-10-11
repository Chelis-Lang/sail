#!/usr/bin/env python3
"""Create owned, byte-identical product copies and audit Linux lease prerequisites.

This tool provides installation evidence only, never freshness or proof authority.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import stat
import sys

NAMES = ("sail", "sail-z3")
MAX_FILE = 128 * 1024 * 1024
MAX_JSON = 64 * 1024 * 1024
MAX_ROWS = 4096
MAX_PATH = 4096
CHUNK = 1024 * 1024
CAP_LEASE = 1 << 28


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("duplicate JSON field")
        result[key] = value
    return result


def open_regular(path, dir_fd=None):
    descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=dir_fd)
    if not stat.S_ISREG(os.fstat(descriptor).st_mode):
        os.close(descriptor)
        raise ValueError("expected regular file")
    return descriptor


def load_json(path, limit=MAX_JSON):
    with os.fdopen(open_regular(path), "rb") as stream:
        data = stream.read(limit + 1)
    if len(data) > limit:
        raise ValueError("JSON byte bound exceeded")
    return json.loads(data, object_pairs_hook=unique_object), hashlib.sha256(data).hexdigest()


def identity():
    """Read actual Linux filesystem UID and effective capability, without guessing."""
    with open("/proc/self/status", "rb") as stream:
        data = stream.read(65537)
    if len(data) > 65536:
        raise ValueError("process status byte bound exceeded")
    fields = {}
    for line in data.decode("ascii").splitlines():
        name, _, value = line.partition(":")
        if name in ("Uid", "Gid", "CapEff"):
            if name in fields:
                raise ValueError("duplicate process status field")
            fields[name] = value.split()
    if set(fields) != {"Uid", "Gid", "CapEff"} or len(fields["Uid"]) != 4 or len(fields["Gid"]) != 4 or len(fields["CapEff"]) != 1:
        raise ValueError("incomplete Linux credential evidence")
    uids = list(map(int, fields["Uid"]))
    gids = list(map(int, fields["Gid"]))
    effective = int(fields["CapEff"][0], 16)
    return {"uids": uids, "gids": gids, "fsuid": uids[3], "cap_eff": fields["CapEff"][0],
            "cap_lease": bool(effective & CAP_LEASE)}


def expected_files(value):
    """Validate the explicit complete two-binary content grant."""
    if not isinstance(value, dict) or set(value) != set(NAMES):
        raise ValueError("expected exactly sail and sail-z3 content grants")
    for row in value.values():
        if not isinstance(row, dict) or set(row) != {"bytes", "sha256"}:
            raise ValueError("invalid content grant fields")
        if type(row["bytes"]) is not int or not 0 < row["bytes"] <= MAX_FILE:
            raise ValueError("binary byte bound exceeded")
        digest = row["sha256"]
        if not isinstance(digest, str) or len(digest) != 64 or any(c not in "0123456789abcdef" for c in digest):
            raise ValueError("invalid SHA-256")
    return value


def digest_fd(descriptor, expected):
    digest = hashlib.sha256()
    count = 0
    while chunk := os.read(descriptor, min(CHUNK, expected["bytes"] + 1 - count)):
        count += len(chunk)
        if count > expected["bytes"]:
            raise ValueError("binary exceeds granted size")
        digest.update(chunk)
    if count != expected["bytes"] or digest.hexdigest() != expected["sha256"]:
        raise ValueError("binary content does not match grant")
    return {"bytes": count, "sha256": digest.hexdigest()}


def install(source, destination, expected):
    """Copy into a new private directory; refuse replacement and preserve sources."""
    expected = expected_files(expected)
    who = identity()
    destination = Path(destination)
    if destination.name in ("", ".", ".."):
        raise ValueError("invalid destination name")
    parent = os.open(destination.parent, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    directory = None
    created = []
    owned = None
    try:
        os.mkdir(destination.name, mode=0o700, dir_fd=parent)
        owned = os.stat(destination.name, dir_fd=parent, follow_symlinks=False)
        directory = os.open(destination.name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=parent)
        opened = os.fstat(directory)
        if (opened.st_dev, opened.st_ino) != (owned.st_dev, owned.st_ino):
            raise ValueError("new directory changed during installation")
        os.fchmod(directory, 0o700)
        if owned.st_uid != who["fsuid"]:
            raise ValueError("created directory ownership differs from filesystem UID")
        files = {}
        for name in NAMES:
            src = open_regular(Path(source) / name)
            try:
                if os.fstat(src).st_size != expected[name]["bytes"]:
                    raise ValueError("source size differs from grant")
                out = os.open(name, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o700, dir_fd=directory)
                created.append(name)
                with os.fdopen(out, "wb") as stream:
                    digest = hashlib.sha256()
                    count = 0
                    while chunk := os.read(src, min(CHUNK, expected[name]["bytes"] + 1 - count)):
                        count += len(chunk)
                        if count > expected[name]["bytes"]:
                            raise ValueError("source grew beyond grant")
                        digest.update(chunk)
                        stream.write(chunk)
                    if count != expected[name]["bytes"] or digest.hexdigest() != expected[name]["sha256"]:
                        raise ValueError("source content does not match grant")
                    os.fchmod(stream.fileno(), 0o700)
                    stream.flush()
                    os.fsync(stream.fileno())
            finally:
                os.close(src)
            check = open_regular(name, dir_fd=directory)
            try:
                files[name] = digest_fd(check, expected[name])
                metadata = os.fstat(check)
                if metadata.st_uid != who["fsuid"] or stat.S_IMODE(metadata.st_mode) != 0o700:
                    raise ValueError("installed ownership or mode differs")
                files[name].update(uid=metadata.st_uid, gid=metadata.st_gid, mode="0700")
            finally:
                os.close(check)
        os.fsync(directory)
        os.fsync(parent)
        return {"version": 1, "authority": "installation-only", "credentials": who, "files": files,
                "directory_mode": "0700", "source_modified": False}
    except BaseException as error:
        # Never replace the primary failure or touch an existing/substituted directory.
        cleanup_errors = []
        if directory is not None:
            for name in created:
                try:
                    os.unlink(name, dir_fd=directory)
                except FileNotFoundError:
                    pass
                except OSError as cleanup:
                    cleanup_errors.append(str(cleanup))
        if owned is not None:
            try:
                current = os.stat(destination.name, dir_fd=parent, follow_symlinks=False)
                if (current.st_dev, current.st_ino) == (owned.st_dev, owned.st_ino):
                    os.rmdir(destination.name, dir_fd=parent)
            except FileNotFoundError:
                pass
            except OSError as cleanup:
                cleanup_errors.append(str(cleanup))
        if cleanup_errors:
            print("error[SAIL0948]: partial-install cleanup: " + "; ".join(cleanup_errors), file=sys.stderr)
        raise error
    finally:
        if directory is not None:
            os.close(directory)
        os.close(parent)


def raw_path(value):
    if not isinstance(value, list) or not 0 < len(value) < MAX_PATH or any(type(b) is not int or not 0 <= b <= 255 for b in value):
        raise ValueError("invalid native path encoding")
    path = bytes(value)
    if not path.startswith(b"/") or b"\0" in path:
        raise ValueError("expected absolute native path")
    return path


def audit_bindings(rows, credentials=None):
    """Check ALL supplied exact bindings; completeness comes from the Guard capture.

    Matching ownership/capability does not establish lease availability or Current.
    """
    if not isinstance(rows, list) or not 0 < len(rows) <= MAX_ROWS:
        raise ValueError("guard inventory row bound exceeded")
    who = identity() if credentials is None else credentials
    seen = set()
    foreign = []
    counts = {}
    for row in rows:
        if not isinstance(row, dict) or set(row) != {"path", "canonical", "device", "inode"}:
            raise ValueError("invalid exact Guard binding")
        path, canonical = raw_path(row["path"]), raw_path(row["canonical"])
        if path in seen:
            raise ValueError("duplicate Guard path")
        seen.add(path)
        if any(type(row[key]) is not int or row[key] < 0 for key in ("device", "inode")):
            raise ValueError("invalid device/inode identity")
        if os.path.realpath(path) != canonical:
            raise ValueError("canonical path changed since capture")
        fd = open_regular(canonical)
        try:
            current = os.fstat(fd)
            alias = os.stat(path)
            bound = (row["device"], row["inode"])
            if (current.st_dev, current.st_ino) != bound or (alias.st_dev, alias.st_ino) != bound:
                raise ValueError("file binding changed since capture")
            counts[str(current.st_uid)] = counts.get(str(current.st_uid), 0) + 1
            if current.st_uid != who["fsuid"]:
                foreign.append({"path": row["path"], "uid": current.st_uid, "device": current.st_dev, "inode": current.st_ino})
        finally:
            os.close(fd)
    return {"version": 1, "authority": "environment-prerequisite-only", "credentials": who,
            "supplied_bindings": len(rows), "uid_counts": counts, "foreign_owned": foreign,
            "ownership_prerequisite_met": not foreign or who["cap_lease"],
            "completeness": "must be established by exact candidate original Guard capture",
            "currentness_certified": False}


def audit_products(directory):
    rows = []
    for name in NAMES:
        path = os.fsencode(Path(directory).absolute() / name)
        canonical = os.path.realpath(path)
        descriptor = open_regular(path)
        try:
            meta = os.fstat(descriptor)
            rows.append({"path": list(path), "canonical": list(canonical), "device": meta.st_dev, "inode": meta.st_ino})
        finally:
            os.close(descriptor)
    result = audit_bindings(rows)
    result["completeness"] = "product binaries only; all runtime/source/helper bindings remain required"
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    create = commands.add_parser("install")
    create.add_argument("--source", type=Path, required=True)
    create.add_argument("--destination", type=Path, required=True)
    create.add_argument("--expected", type=Path, required=True, help="exact {sail,sail-z3}: {bytes,sha256} grants")
    audit = commands.add_parser("audit")
    audit.add_argument("--inventory", type=Path, required=True, help="complete original Guard binding array for this exact candidate")
    args = parser.parse_args()
    try:
        if args.command == "install":
            expected, _ = load_json(args.expected, 4096)
            result = install(args.source, args.destination, expected)
        else:
            rows, digest = load_json(args.inventory)
            result = audit_bindings(rows)
            result["inventory_sha256"] = digest
        print(json.dumps(result, sort_keys=True))
        return 0 if result.get("ownership_prerequisite_met", True) else 1
    except (OSError, ValueError) as error:
        print(f"error[SAIL0948]: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
