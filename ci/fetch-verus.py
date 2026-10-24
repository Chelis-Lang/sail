"""Installs the pinned Verus release for Linux x86-64 into DEST.

Reads corpus/toolchain.toml, downloads the archive, checks its SHA-256 and the
bundled z3's SHA-256, and unpacks it. Used when building the CI image
(ci/Dockerfile); `cargo xtask fetch-verus` does the same on a host.
Usage: fetch-verus.py <toolchain.toml> <dest>
"""

import hashlib
import sys
import tomllib
import urllib.request
import zipfile
from pathlib import Path

ASSET = "x86_64-unknown-linux-gnu"


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def main() -> None:
    pins = tomllib.loads(Path(sys.argv[1]).read_text())
    dest = Path(sys.argv[2])
    asset = pins["assets"][ASSET]
    dest.mkdir(parents=True, exist_ok=True)
    archive = dest / "verus.zip"
    urllib.request.urlretrieve(asset["url"], archive)
    got = sha256(archive)
    if got != asset["sha256"]:
        sys.exit(f"archive SHA-256 {got}, expected {asset['sha256']}")
    with zipfile.ZipFile(archive) as z:
        z.extractall(dest)
    # Direct stock proof checks revalidate this exact archive and all extracted
    # members. Retain it in the CI image as independent runtime provenance.
    install = dest / asset["dir"]
    for f in install.iterdir():
        if f.is_file():
            f.chmod(0o755)
    z3 = sha256(install / "z3")
    if asset["z3_sha256"] and z3 != asset["z3_sha256"]:
        sys.exit(f"z3 SHA-256 {z3}, expected {asset['z3_sha256']}")
    print(f"Verus {pins['verus']['release']} installed in {install}")


if __name__ == "__main__":
    main()
