#!/usr/bin/env python3
"""Materialize original pinned Kani/Rust payloads without executing installers.

Contributor runtime preparation only: no backend selection, source closure,
proof receipt, currentness or result reuse. Inputs and parent must be owned and
immutable throughout. This is not protection against same-user mutation.
"""
from pathlib import Path
import argparse
import hashlib
import importlib.util
import json
import os
import shutil
import stat
import sys
import tarfile
import tempfile


def helper(name):
    """Load only the adjacent tracked helper, never a PATH/PYTHONPATH module."""
    path = Path(__file__).resolve().with_name(name + ".py")
    spec = importlib.util.spec_from_file_location("sail_" + name.replace("-", "_"), path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


rust = helper("kani-toolchain")
bundle = helper("kani-bundle")


def materialize_component(archive, row, files, root, retained):
    """Copy only authenticated payload names and bytes into a new private tree."""
    seen = set()
    with tarfile.open(archive, "r|xz") as package:
        for member in package:
            name = member.name.rstrip("/") if member.isdir() else member.name
            path = rust.relative(name)
            if name in seen or not (member.isfile() or member.isdir()):
                raise ValueError("duplicate, linked or special Rust archive member")
            seen.add(name)
            if len(seen) > rust.MAX_FILES * 2:
                raise ValueError("excessive Rust archive member count")
            if len(path.parts) < 3 or path.parts[1] != row["component"] or path.parts[2:] == ("manifest.in",) or member.isdir():
                continue
            key = str(rust.relative(str(Path(*path.parts[2:]))))
            record = files.get(key)
            if record is None or member.size != record["bytes"] or bool(member.mode & 0o111) != record["executable"]:
                raise ValueError("Rust materialization differs from authenticated payload")
            output = root.joinpath(*path.parts[2:])
            value = hashlib.sha256()
            remaining = member.size
            source = package.extractfile(member)
            target = None
            try:
                if key not in retained:
                    output.parent.mkdir(parents=True, exist_ok=True)
                    target = output.open("xb")
                while remaining:
                    block = source.read(min(128 * 1024, remaining))
                    if not block:
                        raise ValueError("truncated Rust materialization payload")
                    remaining -= len(block)
                    value.update(block)
                    if target is not None:
                        target.write(block)
                if source.read(1) or value.hexdigest() != record["sha256"]:
                    raise ValueError("Rust materialization payload digest differs")
            finally:
                source.close()
                if target is not None:
                    target.close()
            if key not in retained:
                output.chmod(0o755 if record["executable"] else 0o644)
                retained.add(key)


def install(bundle_archive, bundle_pin, archives, rust_pin, destination):
    """Publish both exact payload trees together, atomically without replacement."""
    if not destination.is_absolute() or not destination.name or os.path.lexists(destination):
        raise ValueError("installation needs an absolute new destination")
    parent = destination.parent.resolve(strict=True)
    if not parent.is_dir() or stat.S_IMODE(parent.stat().st_mode) & 0o077:
        raise ValueError("installation parent must be private")
    destination = parent / destination.name
    # Validate all original archive bytes before creating any output tree.
    files = rust.inventory(rust_pin, archives)
    stage = Path(tempfile.mkdtemp(prefix=".sail-kani-install-", dir=parent))
    try:
        bundle.unpack(bundle_archive, bundle_pin, stage / "bundle")
        sysroot = stage / "rust"
        sysroot.mkdir(mode=0o700)
        retained = set()
        for row in rust_pin["components"]:
            materialize_component(archives / row["url"].rsplit("/", 1)[-1], row, files, sysroot, retained)
        if retained != set(files) or rust.verify_installed(sysroot, rust_pin, files):
            raise ValueError("materialized Rust payload is incomplete or has generated metadata")
        bundle.publish(stage, destination)
        stage = None
    finally:
        if stage is not None:
            shutil.rmtree(stage)
    return {"kind": "sail-kani-original-payload-install", "version": 1,
            "toolchain": rust_pin["toolchain"], "rust_payload_files": len(files),
            "rust_payload_bytes": sum(row["bytes"] for row in files.values()),
            "backend_selected": False, "proof_authority": False,
            "scope": "Payload preparation only. Executes no installer code; trusts no generated Rustup metadata; establishes no source closure, currentness, receipt, proof or result reuse."}


def main():
    """Prepare supplied pinned archives; downloading and product setup stay separate."""
    parser = argparse.ArgumentParser(description=__doc__)
    base = Path(__file__).resolve().parents[1] / "engine/kani"
    # The fork bundle is Sail's runtime; bundle-linux.json is stock Kani, the
    # conformance oracle (ADR 0187).
    parser.add_argument("--bundle-pin", type=Path, default=base / "fork-bundle-linux.json")
    parser.add_argument("--rust-pin", type=Path, default=base / "toolchain-linux.json")
    parser.add_argument("--bundle-archive", type=Path, required=True)
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--archives", type=Path, required=True)
    parser.add_argument("--destination", type=Path, required=True)
    args = parser.parse_args()
    if sys.platform != "linux" or os.uname().machine != "x86_64":
        raise ValueError("original payload installation supports native Linux x86-64 only")
    result = install(args.bundle_archive, bundle.load_pin(args.bundle_pin), args.archives,
                     rust.load_pin(args.rust_pin, args.manifest), args.destination)
    print(json.dumps(result, sort_keys=True))


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, KeyError, TypeError, RecursionError, tarfile.TarError) as error:
        print(f"error[SAIL6305]: original Kani payload installation: {error}", file=sys.stderr)
        raise SystemExit(2) from None
