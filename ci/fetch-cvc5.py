#!/usr/bin/env python3
"""Install the pinned contributor-test cvc5 and its bundled notices."""
import argparse
import hashlib
from pathlib import Path, PurePosixPath
import re
import shutil
import stat
import sys
import tempfile
import tomllib
import urllib.request
import zipfile

MAX_ARCHIVE_BYTES = 128 * 1024 * 1024
MAX_FILE_BYTES = 64 * 1024 * 1024
HOST = "x86_64-unknown-linux-gnu"


def sha(path):
    with Path(path).open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def install(pins, destination, archive=None, host=HOST):
    """Validate both digests before writing the curated executable/notices."""
    table = tomllib.loads(Path(pins).read_text())["cvc5"]
    asset = table["assets"][host]
    if (not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", table["version"])
            or table["tag"] != "cvc5-" + table["version"]
            or asset["url"] != "https://github.com/cvc5/cvc5/releases/download/"
            + table["tag"] + "/cvc5-Linux-x86_64-static.zip"
            or host != HOST
            or any(not re.fullmatch(r"[0-9a-f]{64}", asset[key])
                   for key in ["sha256", "bin_sha256"])
            or asset["bin"] != "cvc5-Linux-x86_64-static/bin/cvc5"):
        raise ValueError("invalid cvc5 version, host, URL or digest pin")
    with tempfile.TemporaryDirectory(prefix="sail-cvc5-") as temporary:
        if archive is None:
            archive = Path(temporary) / "runtime.zip"
            with urllib.request.urlopen(asset["url"], timeout=60) as source, archive.open("xb") as output:
                total = 0
                while chunk := source.read(65536):
                    total += len(chunk)
                    if total > MAX_ARCHIVE_BYTES:
                        raise ValueError("cvc5 archive exceeds its byte bound")
                    output.write(chunk)
        archive = Path(archive)
        if archive.stat().st_size > MAX_ARCHIVE_BYTES or sha(archive) != asset["sha256"]:
            raise ValueError("cvc5 archive digest or byte bound mismatch")
        with zipfile.ZipFile(archive) as source:
            prefix = "cvc5-Linux-x86_64-static/"
            names = source.namelist()
            if len(names) != len(set(names)):
                raise ValueError("duplicate cvc5 archive members")
            wanted = [name for name in names if name == asset["bin"]
                      or name in [prefix + "COPYING", prefix + "AUTHORS"]
                      or (name.startswith(prefix + "licenses/") and not name.endswith("/"))]
            if (asset["bin"] not in wanted or prefix + "COPYING" not in wanted
                    or prefix + "AUTHORS" not in wanted
                    or not any(name.startswith(prefix + "licenses/") for name in wanted)):
                raise ValueError("cvc5 executable or bundled notices are missing")
            prepared = []
            retained = 0
            for name in wanted:
                relative = PurePosixPath(name.removeprefix(prefix))
                entry = source.getinfo(name)
                mode = entry.external_attr >> 16
                if (relative.is_absolute() or ".." in relative.parts or "\\" in name
                        or entry.file_size > MAX_FILE_BYTES
                        or stat.S_IFMT(mode) not in [0, stat.S_IFREG]):
                    raise ValueError("invalid or excessive cvc5 member")
                retained += entry.file_size
                if retained > MAX_ARCHIVE_BYTES:
                    raise ValueError("cvc5 curated files exceed their byte bound")
                data = source.read(entry)
                if len(data) != entry.file_size:
                    raise ValueError("cvc5 member length mismatch")
                if name == asset["bin"] and hashlib.sha256(data).hexdigest() != asset["bin_sha256"]:
                    raise ValueError("cvc5 executable digest mismatch")
                staged = Path(temporary) / "files" / relative
                staged.parent.mkdir(parents=True, exist_ok=True)
                staged.write_bytes(data)
                prepared.append((relative, staged))
            destination = Path(destination)
            if destination.exists() or destination.is_symlink():
                raise ValueError("cvc5 destination must be absent")
            # Publish only after every curated file has passed validation.
            shutil.move(str(Path(temporary) / "files"), destination)
            for relative, _ in prepared:
                (destination / relative).chmod(0o755 if relative == PurePosixPath("bin/cvc5") else 0o644)
    return destination / "bin/cvc5"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("pins", type=Path)
    parser.add_argument("destination", type=Path)
    parser.add_argument("--archive", type=Path)
    args = parser.parse_args()
    print(install(args.pins, args.destination, args.archive))


if __name__ == "__main__":
    try:
        main()
    except (ValueError, KeyError, OSError, zipfile.BadZipFile) as error:
        print("error[SAIL0945]: " + str(error), file=sys.stderr)
        raise SystemExit(1) from error
