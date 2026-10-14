#!/usr/bin/env python3
"""Package and verify a pinned engine runtime without compiling it in CI.

The archive is trusted only through the committed archive digest and the exact
Verus fork commit (engine/verus/fork.toml) and lock pins. Artifact manifests
additionally bind every unpacked runtime file.
"""

import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import shutil
import tarfile
import re
import tempfile
import urllib.request

REPO = Path(__file__).resolve().parent.parent
REQUIRED = {"cargo-verus", "verus", "rust_verify", "z3", "vstd.vir"}
RUNTIME = PurePosixPath("verus/source/target-verus/release")
MAX_ARCHIVE_BYTES = 2 * 1024 * 1024 * 1024
RELEASE_NAME = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]{0,127}")
# vstd built like the stock release (sail_verus::engine::VSTD_BUILD).
VSTD_BUILD = "vstd-build:cargo-verus-release"


def sha(path):
    digest = hashlib.sha256()
    with Path(path).open("rb") as source:
        for chunk in iter(lambda: source.read(65536), b""):
            digest.update(chunk)
    return digest.hexdigest()


def fork_value(key):
    text = (REPO / "engine/verus/fork.toml").read_text()
    values = re.findall(r'^' + key + r'\s*=\s*"([^"]*)"\s*$', text, re.M)
    if len(values) != 1:
        raise ValueError("SAIL0944: engine/verus/fork.toml must pin one " + key)
    return values[0]


def fork_repo():
    """The fork repository as `owner/name` (engine/verus/fork.toml); its public
    GitHub releases carry the prebuilt engine runtimes."""
    match = re.fullmatch(r"https://github\.com/([A-Za-z0-9-]+/[A-Za-z0-9._-]+)", fork_value("repo"))
    if match is None:
        raise ValueError("SAIL0944: engine/verus/fork.toml repo must be a GitHub repository URL")
    return match.group(1)


def fork_commit():
    """The exact Verus fork commit the engine is built from (engine/verus/fork.toml)."""
    text = (REPO / "engine/verus/fork.toml").read_text()
    commits = re.findall(r'^commit\s*=\s*"([0-9a-f]{40})"\s*$', text, re.M)
    if len(commits) != 1:
        raise ValueError("SAIL0944: engine/verus/fork.toml must pin one full commit SHA")
    return commits[0]


def safe_path(path):
    name = PurePosixPath(path)
    if not path or name.is_absolute() or any(part in (".", "..") for part in name.parts):
        raise ValueError("SAIL0944: unsafe artifact path")
    return name


def verus_features(features):
    """Verus Cargo features in a manifest's features: the plain names, not the
    `prefix:value` variant tags (sail_verus::engine::cargo_features)."""
    return [feature for feature in features if ":" not in feature]


# Runtime platforms, named by the release tag's suffix
# (engine-runtime-<fork commit>-<platform>), and the host each is built for.
PLATFORMS = {"linux": "x86_64-unknown-linux-gnu", "macos": "aarch64-apple-darwin"}


def pin_host(pin):
    """The host a pin's runtime is built for, from its release tag's platform."""
    platform = pin.get("release_tag", "-linux").rsplit("-", 1)[-1]
    if platform not in PLATFORMS:
        raise ValueError("SAIL0944: unknown engine runtime platform: " + platform)
    return PLATFORMS[platform]


def validate(build, pin):
    manifest = json.loads((build / "sail-build.json").read_text())
    # The pin records the runtime's Verus Cargo features (none today, like the
    # stock release); projects needing another fall back to stock (SAIL3124).
    expected = ["sail-fork:" + pin["fork_commit"], "host:" + pin_host(pin),
                "source-lock:" + pin["source_lock_sha256"], VSTD_BUILD]
    tags = [feature for feature in manifest["features"] if ":" in feature]
    if (manifest["schema_version"] != 1 or manifest["commit"] != pin["commit"]
            or manifest["rust_toolchain"] != pin["rust_toolchain"]
            or tags != expected or manifest["z3_sha256"] != pin["z3_sha256"]
            or verus_features(manifest["features"]) != pin["verus_features"]):
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
    pin = {"schema_version": 2, "variant": build.name, "commit": manifest["commit"],
           "rust_toolchain": manifest["rust_toolchain"], "z3_sha256": manifest["z3_sha256"],
           "fork_commit": fork_commit(),
           "source_lock_sha256": sha(REPO / "engine/verus/Cargo.lock"),
           "verus_features": verus_features(manifest["features"]),
           "release_tag": "engine-runtime-" + fork_commit()[:8] + "-" + getattr(args, "platform", "linux")}
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
    # Published as this asset of the fork's release for the commit
    # (engine/verus/README.md, "Publishing a runtime").
    # The release tag (set above) names the commit and platform.
    pin.update(repo=fork_repo(), release_tag=pin.pop("release_tag"), asset=args.archive.name)
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
    if (pin["schema_version"] != 2 or pin["fork_commit"] != fork_commit()
            or not isinstance(pin.get("verus_features"), list)
            or pin["source_lock_sha256"] != sha(REPO / "engine/verus/Cargo.lock")):
        raise ValueError("SAIL0944: committed engine artifact is stale; rebuild locally")
    return pin


def asset_url(pin):
    """The public release asset on the fork repository (engine/verus/fork.toml).

    The URL is built from the committed fork repository, never taken from the
    pin, and the release tag must name the pinned fork commit. No credential is
    needed or sent; the archive digest and runtime manifest are the trust root.
    """
    tag, asset = pin["release_tag"], pin["asset"]
    if (pin["repo"] != fork_repo() or not RELEASE_NAME.fullmatch(tag) or not RELEASE_NAME.fullmatch(asset)
            or not tag.startswith("engine-runtime-" + pin["fork_commit"][:8] + "-")):
        raise ValueError("SAIL0944: engine release must be a fork release named for the pinned commit")
    return "https://github.com/" + pin["repo"] + "/releases/download/" + tag + "/" + asset


def fetch(args):
    pin = committed_pin(args.pin)
    request = urllib.request.Request(asset_url(pin), headers={"User-Agent": "sail-engine-installer"})
    class Redirect(urllib.request.HTTPRedirectHandler):
        def redirect_request(self, req, fp, code, msg, response_headers, newurl):
            if not newurl.startswith("https://"):
                raise ValueError("SAIL0944: insecure engine asset redirect")
            return super().redirect_request(req, fp, code, msg, response_headers, newurl)
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
    package.add_argument("--platform", choices=sorted(PLATFORMS), default="linux")
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
