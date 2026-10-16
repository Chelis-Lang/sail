#!/usr/bin/env python3
"""Partition an already validated installation without changing runtime files.

Each regular-file hardlink group stays in one part. Every part includes the
original directories so consecutive Docker COPY operations preserve their
metadata, including empty directories. This changes packaging, not the runtime
or its validation. It makes no assertion about compressed size or pull latency.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import stat


def inventory(root):
    """Describe every path and its internal hardlink topology without following links."""
    root = Path(root)
    if root.is_symlink() or not root.is_dir():
        raise ValueError("installation root must be an ordinary directory")
    rows, links = {}, {}
    pending = [root]
    while pending:
        path = pending.pop()
        metadata = path.lstat()
        name = path.relative_to(root).as_posix()
        row = dict(mode=stat.S_IMODE(metadata.st_mode), uid=metadata.st_uid,
                   gid=metadata.st_gid)
        if stat.S_ISDIR(metadata.st_mode):
            row["type"] = "directory"
            pending.extend(sorted(path.iterdir(), reverse=True))
        elif stat.S_ISREG(metadata.st_mode):
            row.update(type="file", bytes=metadata.st_size)
            with path.open("rb") as stream:
                row["sha256"] = hashlib.file_digest(stream, "sha256").hexdigest()
            links.setdefault((metadata.st_dev, metadata.st_ino), []).append(name)
        elif stat.S_ISLNK(metadata.st_mode):
            row.update(type="symlink", target=os.readlink(path))
        else:
            raise ValueError("special installation file refused: " + name)
        rows[name] = row
    for names in links.values():
        if len(names) > 1:
            for name in names:
                rows[name]["hardlinks"] = sorted(names)
    return dict(sorted(rows.items()))


def partition(source, destination, count):
    """Copy deterministic balanced parts and verify their complete, disjoint inventory."""
    if not 1 <= count <= 32:
        raise ValueError("part count must be between 1 and 32")
    source, destination = Path(source), Path(destination)
    if destination.exists() or destination.is_symlink():
        raise ValueError("partition destination must be new")
    if destination.resolve().is_relative_to(source.resolve()):
        raise ValueError("partition destination must be outside the installation")
    before = inventory(source)
    directories = [name for name, row in before.items() if row["type"] == "directory"]
    groups = {}
    for name, row in before.items():
        if row["type"] != "directory":
            groups.setdefault(tuple(row.get("hardlinks", [name])), []).append(name)
    buckets = [[] for _ in range(count)]
    sizes = [0] * count
    ordered = sorted(groups.values(), key=lambda names: (
        -sum(before[name].get("bytes", 0) for name in names), sorted(names)))
    for names in ordered:
        index = min(range(count), key=lambda i: (sizes[i], i))
        buckets[index].extend(names)
        sizes[index] += sum(before[name].get("bytes", 0) for name in names)
    assigned = [name for bucket in buckets for name in bucket]
    expected = {name for name, row in before.items() if row["type"] != "directory"}
    if len(assigned) != len(set(assigned)) or set(assigned) != expected:
        raise ValueError("partition has overlapping or missing installation paths")

    def metadata(original, target):
        info = original.lstat()
        current = target.lstat()
        if (current.st_uid, current.st_gid) != (info.st_uid, info.st_gid):
            os.chown(target, info.st_uid, info.st_gid, follow_symlinks=False)
        shutil.copystat(original, target, follow_symlinks=False)

    destination.mkdir()
    try:
        for index, names in enumerate(buckets):
            root = destination / str(index)
            for name in sorted(directories, key=lambda n: (len(Path(n).parts), n)):
                (root / name).mkdir(parents=True, exist_ok=True)
            copied = {}
            for name in sorted(names):
                original, target = source / name, root / name
                row = before[name]
                if row["type"] == "symlink":
                    target.symlink_to(row["target"])
                else:
                    group = tuple(row.get("hardlinks", [name]))
                    if group in copied:
                        os.link(copied[group], target)
                    else:
                        shutil.copyfile(original, target, follow_symlinks=False)
                        copied[group] = target
                metadata(original, target)
            for name in sorted(directories, key=lambda n: (-len(Path(n).parts), n)):
                metadata(source / name, root / name)
            actual = inventory(root)
            wanted = {name: before[name] for name in [*directories, *names]}
            if actual != wanted:
                raise ValueError("partition changed installation identity: " + str(index))
        if inventory(source) != before:
            raise ValueError("installation changed while partitioning")
        manifest = dict(schema_version=1, scope="packaging only; no latency acceptance",
                        files=before, parts=[dict(index=i, file_bytes=sizes[i],
                        paths=sorted(names)) for i, names in enumerate(buckets)])
        (destination / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
        return manifest
    except BaseException:
        # Keep partial data for diagnosis; no caller may COPY it after failure.
        raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    part = sub.add_parser("partition")
    part.add_argument("source", type=Path)
    part.add_argument("destination", type=Path)
    part.add_argument("--parts", type=int, default=4)
    check = sub.add_parser("inventory")
    check.add_argument("source", type=Path)
    args = parser.parse_args()
    if args.command == "partition":
        manifest = partition(args.source, args.destination, args.parts)
        result = dict(paths=len(manifest["files"]), scope=manifest["scope"],
                      parts=[dict(index=p["index"], file_bytes=p["file_bytes"],
                                  paths=len(p["paths"])) for p in manifest["parts"]])
    else:
        result = inventory(args.source)
    print(json.dumps(result, sort_keys=True))


if __name__ == "__main__":
    main()
