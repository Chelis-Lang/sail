#!/usr/bin/env python3
"""Read-only verification of the original Rust payload used by the Kani fixture.

This does not install, execute or select Kani/Rust, authenticate generated rustup
metadata, or establish source closure, currentness, a proof or cache authority.
"""
from pathlib import Path, PurePosixPath
import argparse
import hashlib
import json
import os
import stat
import sys
import tarfile
import tomllib

HOST = "x86_64-unknown-linux-gnu"
DATE = "2026-08-21"
MANIFEST_SHA256 = "3c30cc655a7549d0b30015cd032f949e8723da5ad039f672d04520bbaea4e76d"
PACKAGES = {"cargo", "rustc", "rust-std", "rustc-dev", "llvm-tools-preview", "rust-src"}
MAX_ARCHIVE = 160 * 1024 * 1024
MAX_PAYLOAD = 2 * 1024 * 1024 * 1024
MAX_FILES = 10_000


def sha(stream):
    """Hash original bytes with a fixed-size buffer."""
    h = hashlib.sha256()
    for block in iter(lambda: stream.read(128 * 1024), b""):
        h.update(block)
    return h.hexdigest()


def object_pairs(pairs):
    """Reject duplicate JSON fields."""
    out = {}
    for key, value in pairs:
        if key in out:
            raise ValueError("duplicate toolchain pin field")
        out[key] = value
    return out


def relative(name):
    """Require original canonical relative spelling, without following paths."""
    if not isinstance(name, str) or not name or len(name) > 4096 or "\\" in name:
        raise ValueError("unsupported Rust payload path")
    p = PurePosixPath(name)
    if p.is_absolute() or ".." in p.parts or str(p) != name or name == "." or len(p.parts) > 64:
        raise ValueError("noncanonical Rust payload path")
    return p


def load_pin(pin_file, manifest_file):
    """Bind component identities to the exact original official channel file."""
    if pin_file.stat().st_size > 32 * 1024 or manifest_file.stat().st_size > 4 * 1024 * 1024:
        raise ValueError("excessive toolchain metadata")
    pin = json.loads(pin_file.read_bytes(), object_pairs_hook=object_pairs)
    with manifest_file.open("rb") as f:
        if sha(f) != MANIFEST_SHA256:
            raise ValueError("original Rust channel manifest digest differs")
    manifest = tomllib.loads(manifest_file.read_text())
    if (
        not isinstance(pin, dict)
        or set(pin) != {"schema_version", "purpose", "host", "toolchain", "rustc_version", "manifest_url", "manifest_sha256", "components"}
        or type(pin["schema_version"]) is not int or pin["schema_version"] != 1
        or pin["host"] != HOST
        or pin["toolchain"] != f"nightly-{DATE}-{HOST}"
        or pin["rustc_version"] != "rustc 1.100.0-nightly (8925ea358 2026-08-20)"
        or pin["manifest_url"] != f"https://static.rust-lang.org/dist/{DATE}/channel-rust-nightly.toml"
        or pin["manifest_sha256"] != MANIFEST_SHA256
        or manifest["date"] != DATE
        or not isinstance(pin["components"], list) or len(pin["components"]) != len(PACKAGES)
    ):
        raise ValueError("unsupported Rust toolchain identity")
    seen = set()
    for row in pin["components"]:
        if not isinstance(row, dict) or set(row) != {"package", "component", "url", "archive_bytes", "archive_sha256", "version", "payload_files", "payload_bytes"}:
            raise ValueError("unsupported component fields")
        package = row["package"]
        if package not in PACKAGES or package in seen:
            raise ValueError("duplicate or unsupported Rust component")
        seen.add(package)
        expected_component = f"rust-std-{HOST}" if package == "rust-std" else package
        target = manifest["pkg"][package]["target"]["*" if package == "rust-src" else HOST]
        if (
            row["component"] != expected_component or not target["available"]
            or row["url"] != target["xz_url"] or row["archive_sha256"] != target["xz_hash"]
            or row["version"] != manifest["pkg"][package]["version"]
            or not row["url"].startswith(f"https://static.rust-lang.org/dist/{DATE}/")
            or not row["url"].endswith(".tar.xz")
            or type(row["archive_bytes"]) is not int or not 0 < row["archive_bytes"] <= MAX_ARCHIVE
            or type(row["payload_files"]) is not int or not 0 < row["payload_files"] <= MAX_FILES
            or type(row["payload_bytes"]) is not int or not 0 < row["payload_bytes"] <= MAX_PAYLOAD
        ):
            raise ValueError("Rust component does not match original channel metadata")
    if sum(r["payload_files"] for r in pin["components"]) > MAX_FILES or sum(r["payload_bytes"] for r in pin["components"]) > MAX_PAYLOAD:
        raise ValueError("excessive aggregate Rust payload")
    return pin


def component_inventory(archive, row):
    """Describe every regular installed payload byte; never run install.sh."""
    if archive.is_symlink() or not archive.is_file() or archive.stat().st_size != row["archive_bytes"]:
        raise ValueError("missing, linked or wrong-sized original Rust archive")
    with archive.open("rb") as f:
        if sha(f) != row["archive_sha256"]:
            raise ValueError("original Rust archive digest differs")
    files = {}
    seen = set()
    roots = set()
    total = 0
    declared = None
    members = 0
    with tarfile.open(archive, "r|xz") as tar:
        for member in tar:
            members += 1
            if members > MAX_FILES * 2:
                raise ValueError("excessive Rust archive member count")
            name = member.name.rstrip("/") if member.isdir() else member.name
            p = relative(name)
            if name in seen or not (member.isfile() or member.isdir()):
                raise ValueError("duplicate, linked or special Rust archive member")
            seen.add(name)
            roots.add(p.parts[0])
            if len(roots) != 1:
                raise ValueError("multiple Rust archive roots")
            if len(p.parts) == 2 and p.parts[1] == "components":
                if not member.isfile() or member.size > 256:
                    raise ValueError("invalid Rust component declaration")
                declared = tar.extractfile(member).read().decode("ascii").strip()
            if len(p.parts) < 3 or p.parts[1] != row["component"] or p.parts[2:] == ("manifest.in",):
                continue
            if member.isdir():
                continue
            destination = str(PurePosixPath(*p.parts[2:]))
            relative(destination)
            total += member.size
            if len(files) >= MAX_FILES or total > MAX_PAYLOAD or member.size < 0:
                raise ValueError("Rust payload exceeds fixed bounds")
            files[destination] = {"bytes": member.size, "sha256": sha(tar.extractfile(member)), "executable": bool(member.mode & 0o111)}
    if declared != row["component"] or len(files) != row["payload_files"] or total != row["payload_bytes"]:
        raise ValueError("incomplete Rust component payload")
    return files


def inventory(pin, archives):
    """Combine complete original payloads; conflicting overlays refuse."""
    files = {}
    for row in pin["components"]:
        for name, record in component_inventory(archives / row["url"].rsplit("/", 1)[-1], row).items():
            if name in files and files[name] != record:
                raise ValueError("conflicting original Rust component payloads")
            files[name] = record
    return files


def traversal_error(error):
    """A skipped unreadable subtree cannot establish a complete inventory."""
    raise error


def verify_installed(root, pin, files):
    """Check a caller-owned immutable prefix; generated metadata is not trusted."""
    root = root.resolve(strict=True)
    if not root.is_dir():
        raise ValueError("Rust sysroot is not a directory")
    metadata = {"lib/rustlib/components", "lib/rustlib/rust-installer-version", "lib/rustlib/multirust-channel-manifest.toml", "lib/rustlib/multirust-config.toml"}
    metadata.update(f"lib/rustlib/manifest-{r['package']}" + ("" if r["package"] == "rust-src" else f"-{HOST}") for r in pin["components"])
    seen = set()
    generated = {}
    for directory, dirs, names in os.walk(root, followlinks=False, onerror=traversal_error):
        for name in dirs:
            if not stat.S_ISDIR((Path(directory) / name).lstat().st_mode):
                raise ValueError("linked or special directory in Rust prefix")
        for name in names:
            p = Path(directory) / name
            mode = p.lstat().st_mode
            if not stat.S_ISREG(mode):
                raise ValueError("linked or special file in Rust prefix")
            name = str(p.relative_to(root))
            if len(seen) >= MAX_FILES + len(metadata):
                raise ValueError("excessive installed Rust inventory")
            if name in files:
                expected = files[name]
                if p.stat().st_size != expected["bytes"] or bool(mode & 0o111) != expected["executable"]:
                    raise ValueError(f"installed Rust payload metadata differs: {name}")
                with p.open("rb") as f:
                    if sha(f) != expected["sha256"]:
                        raise ValueError(f"installed Rust payload differs: {name}")
            elif name in metadata and p.stat().st_size <= 4 * 1024 * 1024 and not mode & 0o111:
                with p.open("rb") as f:
                    generated[name] = sha(f)
            else:
                raise ValueError(f"unbound file in Rust prefix: {name}")
            seen.add(name)
    if not set(files) <= seen:
        raise ValueError("installed Rust payload is incomplete")
    return generated


def main():
    """Verify supplied original archives, optionally an installed prefix."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pin", type=Path, default=Path(__file__).resolve().parents[1] / "engine/kani/toolchain-linux.json")
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--archives", type=Path, required=True)
    parser.add_argument("--sysroot", type=Path)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    pin = load_pin(args.pin, args.manifest)
    files = inventory(pin, args.archives)
    generated = verify_installed(args.sysroot, pin, files) if args.sysroot else None
    result = {"kind": "sail-kani-original-rust-payload", "version": 1, "toolchain": pin["toolchain"], "manifest_sha256": pin["manifest_sha256"], "files": files, "generated_metadata_sha256_untrusted": generated, "scope": "Original payload verification only; no executable backend, source closure, currentness, proof or cache authority. Prefix must remain caller-owned and immutable; no concurrent-mutation guarantee."}
    with args.out.open("x") as output:
        json.dump(result, output, indent=2, sort_keys=True)
        output.write("\n")
    with args.out.open("rb") as output:
        digest = sha(output)
    print(json.dumps({"files": len(files), "installed_payload_checked": generated is not None, "inventory_sha256": digest}))


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, KeyError, TypeError, RecursionError, tarfile.TarError) as error:
        print(f"error[SAIL6302]: original Rust payload verification: {error}", file=sys.stderr)
        raise SystemExit(2) from None
