#!/usr/bin/env python3
"""Package and verify a pinned engine runtime without compiling it in CI.

The archive is trusted only through the committed archive digest and exact source
patch/lock pins. Artifact manifests additionally bind every unpacked runtime file.
"""

import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import shutil
import tarfile
import tempfile
import urllib.request

REPO = Path(__file__).resolve().parent.parent
REQUIRED = {"cargo-verus", "verus", "rust_verify", "z3", "vstd.vir"}
RUNTIME = PurePosixPath("verus/source/target-verus/release")
MAX_ARCHIVE_BYTES = 2 * 1024 * 1024 * 1024


def sha(path):
    digest = hashlib.sha256()
    with Path(path).open("rb") as source:
        for chunk in iter(lambda: source.read(65536), b""):
            digest.update(chunk)
    return digest.hexdigest()


def safe_path(path):
    name = PurePosixPath(path)
    if not path or name.is_absolute() or any(part in (".", "..") for part in name.parts):
        raise ValueError("SAIL0944: unsafe artifact path")
    return name


def validate(build, pin):
    manifest = json.loads((build / "sail-build.json").read_text())
    expected = ["sail-interactive:" + pin["patch_sha256"], "host:x86_64-unknown-linux-gnu",
                "source-lock:" + pin["source_lock_sha256"]]
    if (manifest["schema_version"] != 1 or manifest["commit"] != pin["commit"]
            or manifest["rust_toolchain"] != pin["rust_toolchain"]
            or manifest["features"] != expected or manifest["z3_sha256"] != pin["z3_sha256"]):
        raise ValueError("SAIL0944: incompatible engine provenance")
    artifacts = manifest["artifacts"]
    if not artifacts or not {str(RUNTIME / name) for name in REQUIRED}.issubset(artifacts):
        raise ValueError("SAIL0944: incomplete runtime manifest")
    actual = {str(RUNTIME / path.name) for path in (build / RUNTIME).iterdir() if path.is_file()}
    if actual != set(artifacts):
        raise ValueError("SAIL0944: runtime contains an unbound artifact")
    for name, digest in artifacts.items():
        relative = safe_path(name)
        if relative.parent != RUNTIME or sha(build / relative) != digest:
            raise ValueError("SAIL0944: runtime artifact mismatch: " + name)
    if sha(build / RUNTIME / "z3") != pin["z3_sha256"]:
        raise ValueError("SAIL0944: pinned solver mismatch")
    return manifest


def pack(args):
    build = args.build.resolve()
    manifest = json.loads((build / "sail-build.json").read_text())
    pin = {"schema_version": 1, "variant": build.name, "commit": manifest["commit"],
           "rust_toolchain": manifest["rust_toolchain"], "z3_sha256": manifest["z3_sha256"],
           "patch_sha256": sha(REPO / "engine/verus/interactive.patch"),
           "source_lock_sha256": sha(REPO / "engine/verus/Cargo.lock")}
    validate(build, pin)
    args.archive.parent.mkdir(parents=True, exist_ok=True)
    # Canonical ownership, ordering and timestamps make repeated packing stable.
    with args.archive.open("xb") as output, gzip.GzipFile(filename="", fileobj=output, mode="wb", mtime=0) as compressed:
        with tarfile.open(fileobj=compressed, mode="w") as archive:
            for name in ["sail-build.json"] + sorted(manifest["artifacts"]):
                info = archive.gettarinfo(str(build / name), arcname=name)
                if not info.isfile():
                    # Runtime symlinks are materialized, preserving only their bytes.
                    target = (build / name).resolve()
                    info = archive.gettarinfo(str(target), arcname=name)
                info.uid = info.gid = info.mtime = 0
                info.uname = info.gname = ""
                with (build / name).open("rb") as source:
                    archive.addfile(info, source)
    pin["archive_sha256"] = sha(args.archive)
    args.pin.write_text(json.dumps(pin, indent=2) + "\n")
    print(json.dumps(pin, indent=2))


def install(archive, pin, cache):
    if sha(archive) != pin["archive_sha256"]:
        raise ValueError("SAIL0944: engine archive digest mismatch")
    variant = safe_path(pin["variant"])
    if len(variant.parts) != 1:
        raise ValueError("SAIL0944: invalid isolated variant name")
    parent = cache / "verus-src-build"
    parent.mkdir(parents=True, exist_ok=True)
    destination = parent / variant
    if destination.exists():
        validate(destination, pin)
        return destination
    with tempfile.TemporaryDirectory(prefix=".sail-engine-", dir=parent) as staging:
        root = Path(staging)
        total = 0
        with tarfile.open(archive, "r:gz") as source:
            for ordinal, member in enumerate(source):
                relative = safe_path(member.name)
                total += member.size
                if (ordinal >= 256 or total > MAX_ARCHIVE_BYTES or not member.isfile()
                        or (relative != PurePosixPath("sail-build.json") and relative.parent != RUNTIME)):
                    raise ValueError("SAIL0944: invalid or oversized engine archive")
                target = root / relative
                target.parent.mkdir(parents=True, exist_ok=True)
                contents = source.extractfile(member)
                if contents is None:
                    raise ValueError("SAIL0944: unreadable archive member")
                with contents, target.open("xb") as out:
                    shutil.copyfileobj(contents, out, 65536)
                target.chmod(member.mode & 0o755)
        validate(root, pin)
        root.rename(destination)
    return destination


def committed_pin(path):
    pin = json.loads(path.read_text())
    if (pin["schema_version"] != 1 or pin["patch_sha256"] != sha(REPO / "engine/verus/interactive.patch")
            or pin["source_lock_sha256"] != sha(REPO / "engine/verus/Cargo.lock")):
        raise ValueError("SAIL0944: committed engine artifact is stale; rebuild locally")
    return pin


def fetch(args):
    pin = committed_pin(args.pin)
    headers = {"Accept": "application/octet-stream", "User-Agent": "sail-engine-installer"}
    token = os.environ.get("SAIL_ENGINE_ARTIFACT_TOKEN")
    if token:
        headers["Authorization"] = "Bearer " + token
    # The credential is sent only to the fixed repository API, never to an
    # arbitrary pin URL. urllib strips it on the signed asset redirect below.
    url = "https://api.github.com/repos/Chelis-Lang/sail/releases/assets/" + str(int(pin["asset_id"]))
    request = urllib.request.Request(url, headers=headers)
    class Redirect(urllib.request.HTTPRedirectHandler):
        def redirect_request(self, req, fp, code, msg, response_headers, newurl):
            redirected = super().redirect_request(req, fp, code, msg, response_headers, newurl)
            if redirected is not None:
                if not newurl.startswith("https://"):
                    raise ValueError("SAIL0944: insecure engine asset redirect")
                redirected.remove_header("Authorization")
            return redirected
    with tempfile.TemporaryDirectory(prefix="sail-engine-download-") as temporary:
        archive = Path(temporary) / "engine.tar.gz"
        with urllib.request.build_opener(Redirect).open(request, timeout=60) as source, archive.open("xb") as out:
            total = 0
            while True:
                chunk = source.read(65536)
                if not chunk:
                    break
                total += len(chunk)
                if total > MAX_ARCHIVE_BYTES:
                    raise ValueError("SAIL0944: oversized engine download")
                out.write(chunk)
        print(install(archive, pin, args.cache))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    package = commands.add_parser("pack")
    package.add_argument("--build", type=Path, required=True)
    package.add_argument("--archive", type=Path, required=True)
    package.add_argument("--pin", type=Path, required=True)
    download = commands.add_parser("fetch")
    download.add_argument("--pin", type=Path, required=True)
    download.add_argument("--cache", type=Path, required=True)
    unpack = commands.add_parser("install")
    unpack.add_argument("--archive", type=Path, required=True)
    unpack.add_argument("--pin", type=Path, required=True)
    unpack.add_argument("--cache", type=Path, required=True)
    args = parser.parse_args()
    if args.command == "pack":
        if os.environ.get("CI"):
            raise ValueError("SAIL0944: engine packaging/build publication is local-only")
        pack(args)
    elif args.command == "fetch":
        fetch(args)
    else:
        print(install(args.archive, committed_pin(args.pin), args.cache))


if __name__ == "__main__":
    try:
        main()
    except (ValueError, KeyError, OSError, tarfile.TarError) as error:
        raise SystemExit("error[SAIL0944]: " + str(error))
