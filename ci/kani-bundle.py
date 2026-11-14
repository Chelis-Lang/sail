#!/usr/bin/env python3
"""Install the exact Linux Kani release bundle, without selecting a backend."""
from pathlib import Path, PurePosixPath
import argparse
import ctypes
import errno
import hashlib
import json
import os
import shutil
import sys
import tarfile
import tempfile
import urllib.request

MAX_ARCHIVE = 160 * 1024 * 1024
MAX_EXPANDED = 512 * 1024 * 1024
MAX_FILES = 256


def digest(stream):
    """Hash original bytes with bounded memory."""
    value = hashlib.sha256()
    for chunk in iter(lambda: stream.read(128 * 1024), b""):
        value.update(chunk)
    return value.hexdigest()


def object_pairs(pairs):
    """Reject duplicate fields before JSON map construction."""
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("duplicate bundle pin field")
        result[key] = value
    return result


def load_pin(path):
    """Read the contributor pin and reject unsupported producer metadata."""
    if path.stat().st_size > 128 * 1024:
        raise ValueError("excessive Kani bundle pin")
    pin = json.loads(path.read_bytes(), object_pairs_hook=object_pairs)
    if not isinstance(pin, dict):
        raise ValueError("invalid bundle pin document")
    if (
        type(pin.get("schema_version")) is not int
        or pin.get("schema_version") != 1
        or pin.get("version") != "0.68.0"
        or pin.get("cbmc_version") != "6.11.0"
        or pin.get("host") != "x86_64-unknown-linux-gnu"
        or pin.get("rust_toolchain") != "nightly-2026-08-21-x86_64-unknown-linux-gnu"
        or pin.get("rustc_version") != "rustc 1.100.0-nightly (8925ea358 2026-08-20)"
        or pin.get("url") != "https://github.com/model-checking/kani/releases/download/kani-0.68.0/kani-0.68.0-x86_64-unknown-linux-gnu.tar.gz"
    ):
        raise ValueError("unsupported Kani release bundle identity")
    if set(pin) != {"schema_version", "version", "cbmc_version", "host", "url", "archive_bytes", "archive_sha256", "rust_toolchain", "rustc_version", "files"}:
        raise ValueError("unsupported bundle pin fields")
    files = pin.get("files")
    if not isinstance(files, dict) or not 0 < len(files) <= MAX_FILES:
        raise ValueError("missing or excessive bundle inventory")
    if type(pin.get("archive_bytes")) is not int or not 0 < pin["archive_bytes"] <= MAX_ARCHIVE:
        raise ValueError("invalid archive byte bound")
    for name, record in files.items():
        relative(name)
        if not isinstance(record, dict) or not isinstance(record.get("bytes"), int) or isinstance(record["bytes"], bool) or record["bytes"] < 0:
            raise ValueError("invalid runtime file size")
        if type(record.get("executable")) is not bool or set(record) != {"bytes", "sha256", "executable"}:
            raise ValueError("invalid runtime file metadata")
        check_hash(record.get("sha256"))
    names = set(files)
    if any(str(parent) in names for name in files for parent in relative(name).parents):
        raise ValueError("file and directory paths collide")
    check_hash(pin.get("archive_sha256"))
    if sum(row["bytes"] for row in files.values()) > MAX_EXPANDED:
        raise ValueError("excessive expanded bundle")
    return pin


def check_hash(value):
    """Require canonical SHA-256 text."""
    if not isinstance(value, str) or len(value) != 64 or any(c not in "0123456789abcdef" for c in value):
        raise ValueError("invalid SHA-256 identity")


def relative(name):
    """Keep original canonical relative spelling; never normalize aliases."""
    if not isinstance(name, str) or not name or len(name) > 4096 or "\\" in name:
        raise ValueError("invalid bundle path")
    path = PurePosixPath(name)
    if path.is_absolute() or len(path.parts) > 64 or ".." in path.parts or str(path) != name or name == ".":
        raise ValueError("noncanonical bundle path")
    return path


def publish(stage, destination):
    """Atomically refuse every existing Linux destination, including races.

    The fixed libc signature takes two retained NUL-free path-byte buffers and
    scalar constants only. No pointer arithmetic, callbacks or transferred
    memory ownership is involved. Unsupported libc/kernel/filesystem cases
    fail; a check-then-rename fallback would lose the no-replace guarantee.
    """
    source_bytes, destination_bytes = os.fsencode(stage), os.fsencode(destination)
    if b"\0" in source_bytes or b"\0" in destination_bytes:
        raise ValueError("NUL in publication path")
    library = ctypes.CDLL(None, use_errno=True)
    try:
        rename = library.renameat2
    except AttributeError as error:
        raise OSError(errno.ENOSYS, "atomic no-replace rename is unavailable") from error
    rename.argtypes = [ctypes.c_int, ctypes.c_char_p, ctypes.c_int, ctypes.c_char_p, ctypes.c_uint]
    rename.restype = ctypes.c_int
    # Linux AT_FDCWD and RENAME_NOREPLACE. Failure never falls back to rename.
    if rename(-100, source_bytes, -100, destination_bytes, 1) != 0:
        code = ctypes.get_errno()
        raise OSError(code, os.strerror(code), str(destination))


def unpack(archive, pin, destination):
    """Publish a complete private same-filesystem tree in one rename."""
    if os.path.lexists(destination):
        raise ValueError("bundle destination already exists")
    if archive.stat().st_size != pin["archive_bytes"]:
        raise ValueError("archive byte count differs")
    with archive.open("rb") as stream:
        if digest(stream) != pin["archive_sha256"]:
            raise ValueError("archive digest differs")
    expected = pin["files"]
    parents = {PurePosixPath(".")}
    for name in expected:
        parents.update(relative(name).parents)
    destination.parent.mkdir(parents=True, exist_ok=True)
    stage = Path(tempfile.mkdtemp(prefix=".sail-kani-", dir=destination.parent))
    try:
        seen = set()
        retained = set()
        with tarfile.open(archive, "r:gz") as package:
            for member in package:
                spelling = member.name
                if member.isdir() and spelling.endswith("/"):
                    spelling = spelling[:-1]
                path = relative(spelling)
                if path.parts[0] != "kani-0.68.0" or spelling in seen:
                    raise ValueError("foreign or duplicate archive member")
                seen.add(spelling)
                name = PurePosixPath(*path.parts[1:])
                if member.isdir():
                    if name not in parents:
                        raise ValueError("unexpected archive directory")
                    continue
                key = str(name)
                if member.type not in (tarfile.REGTYPE, tarfile.AREGTYPE) or key not in expected or key in retained:
                    raise ValueError("unexpected or nonregular archive file")
                record = expected[key]
                if member.size != record["bytes"] or bool(member.mode & 0o111) != record["executable"]:
                    raise ValueError("runtime file size or executable mode differs")
                output = stage.joinpath(*name.parts)
                output.parent.mkdir(parents=True, exist_ok=True)
                with package.extractfile(member) as source, output.open("xb") as target:
                    remaining = member.size
                    value = hashlib.sha256()
                    while remaining:
                        chunk = source.read(min(128 * 1024, remaining))
                        if not chunk:
                            raise ValueError("truncated archive file")
                        remaining -= len(chunk)
                        value.update(chunk)
                        target.write(chunk)
                    if source.read(1) or value.hexdigest() != record["sha256"]:
                        raise ValueError("runtime file digest differs")
                output.chmod(0o755 if member.mode & 0o111 else 0o644)
                retained.add(key)
        if retained != set(expected):
            raise ValueError("incomplete archive inventory")
        if os.path.lexists(destination):
            raise ValueError("bundle destination appeared during staging")
        for directory in stage.rglob("*"):
            if directory.is_dir():
                directory.chmod(0o755)
        stage.chmod(0o755)
        publish(stage, destination)
        stage = None
    finally:
        if stage is not None and stage.exists():
            shutil.rmtree(stage)


def main():
    """Contributor-only fixture installer; Rust setup/execution stay separate."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pin", type=Path, default=Path(__file__).resolve().parent.parent / "engine/kani/bundle-linux.json")
    parser.add_argument("--archive", type=Path)
    parser.add_argument("--destination", type=Path, required=True)
    args = parser.parse_args()
    if sys.platform != "linux" or os.uname().machine != "x86_64":
        raise ValueError("this fixture pin supports native Linux x86-64 only")
    pin = load_pin(args.pin)
    if args.archive:
        unpack(args.archive, pin, args.destination)
    else:
        with tempfile.TemporaryDirectory(prefix="sail-kani-download-") as work:
            archive = Path(work) / "bundle.tar.gz"
            with urllib.request.urlopen(pin["url"], timeout=60) as response, archive.open("xb") as output:
                remaining = pin["archive_bytes"]
                while remaining:
                    chunk = response.read(min(128 * 1024, remaining))
                    if not chunk:
                        raise ValueError("truncated Kani download")
                    remaining -= len(chunk)
                    output.write(chunk)
                if response.read(1):
                    raise ValueError("excessive Kani download")
            unpack(archive, pin, args.destination)
    print(args.destination)


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, tarfile.TarError) as error:
        print(f"Kani fixture: {error}", file=sys.stderr)
        raise SystemExit(1) from error
