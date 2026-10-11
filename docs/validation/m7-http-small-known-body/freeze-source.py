#!/usr/bin/env python3
"""Prepare the exact Git source archive and inventory; no compiler or runtime."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tarfile


def digest(path):
    value = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            value.update(block)
    return value.hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    payload = Path(__file__).resolve().parent
    inputs = json.loads((payload / "inputs.json").read_text())
    args.output.mkdir(exist_ok=False)
    archive = args.output / "source.tar"
    with archive.open("xb") as handle:
        subprocess.run(["git", "archive", "--format=tar", inputs["candidate_commit"]],
                       cwd=args.repo, stdout=handle, check=True, timeout=60)
    assert archive.stat().st_size <= 2 * 1024 * 1024 * 1024
    files, symlinks = {}, {}
    with tarfile.open(archive, "r:") as handle:
        count = 0
        for member in handle:
            count += 1
            assert count <= 8192
            path = Path(member.name)
            assert not path.is_absolute() and ".." not in path.parts
            assert len(member.name.encode()) <= 4096
            if member.isdir():
                continue
            assert member.name not in files and member.name not in symlinks
            if member.issym():
                assert not Path(member.linkname).is_absolute()
                symlinks[member.name] = member.linkname
            else:
                assert member.isfile() and member.size <= 2 * 1024 * 1024 * 1024
                value = hashlib.sha256()
                with handle.extractfile(member) as stream:
                    for block in iter(lambda: stream.read(1024 * 1024), b""):
                        value.update(block)
                files[member.name] = value.hexdigest()
    for name, expected in inputs["candidate_sources"].items():
        assert files[name] == expected, name
    record = {"git_commit": inputs["candidate_commit"], "archive_sha256": digest(archive),
              "archive_bytes": archive.stat().st_size, "files": files, "symlinks": symlinks}
    (args.output / "source-inventory.json").write_text(json.dumps(record, indent=2) + "\n")


if __name__ == "__main__":
    main()
