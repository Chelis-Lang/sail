#!/usr/bin/env python3
"""Verify and publish the explicitly enabled Sail workspace packages together.

Use --dry-run without credentials. Initial publication accepts --token-file;
release automation supplies CARGO_REGISTRY_TOKEN instead. Existing versions
are resumable only when their registry checksum matches the local .crate.
"""

import argparse
from datetime import datetime, timezone
from email.utils import parsedate_to_datetime
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen


ROOT = Path(__file__).resolve().parent.parent


def public_environment():
    """Keep registry credentials out of all preparation subprocesses."""
    return {
        key: value for key, value in os.environ.items()
        if key != "CARGO_REGISTRY_TOKEN"
        and not (key.startswith("CARGO_REGISTRIES_") and key.endswith("_TOKEN"))
    }


def run(arguments, capture=False):
    """Run a preparation command without passing a publication token."""
    return subprocess.run(
        arguments, cwd=ROOT, env=public_environment(), check=True,
        text=True, stdout=subprocess.PIPE if capture else None,
    ).stdout


def clean_commit():
    """Refuse staged, unstaged or untracked changes and identify the commit."""
    if run(["git", "status", "--porcelain", "--untracked-files=all"], capture=True):
        raise ValueError("publication requires a clean Git worktree")
    return run(["git", "rev-parse", "HEAD"], capture=True).strip()


def select_packages(metadata, expected_version=None):
    """Select only workspace members explicitly allowing crates.io."""
    members = set(metadata["workspace_members"])
    packages = []
    for package in metadata["packages"]:
        if package["id"] not in members:
            continue
        if package.get("publish") == ["crates-io"]:
            if not re.fullmatch(r"sail-[a-z0-9-]+", package["name"]):
                raise ValueError("published workspace packages must have Sail names")
            packages.append(package)
        elif package.get("publish") != []:
            raise ValueError(f"{package['name']} must explicitly enable crates.io or disable publication")
    if not any(package["name"] == "sail-solve" for package in packages):
        raise ValueError("the sail-solve CLI package must be selected")
    versions = {package["version"] for package in packages}
    if len(versions) != 1:
        raise ValueError("published workspace package versions must match")
    version = versions.pop()
    if expected_version is not None and expected_version != version:
        raise ValueError(f"requested version {expected_version} does not match workspace version {version}")
    return sorted(packages, key=lambda package: package["name"]), version


def retry_delay(headers, attempt):
    """Honor numeric or HTTP-date Retry-After, with a bounded retry count."""
    value = headers.get("Retry-After", "")
    if value.isdecimal():
        return max(1, int(value))
    if value:
        try:
            return max(1, (parsedate_to_datetime(value) - datetime.now(timezone.utc)).total_seconds())
        except (ValueError, TypeError):
            pass
    return 2 ** attempt


def registry_version(name, version):
    """Read public sparse-index metadata without authentication."""
    # Every selected package has a validated sail-* name, hence length > 3.
    url = f"https://index.crates.io/{name[:2]}/{name[2:4]}/{name}"
    request = Request(url, headers={
        "User-Agent": "sail-crate-publisher (https://github.com/Chelis-Lang/sail)",
        "Cache-Control": "no-cache",
    })
    for attempt in range(6):
        try:
            with urlopen(request, timeout=30) as response:
                entries = [json.loads(line) for line in response if line.strip()]
            matches = [entry for entry in entries if entry.get("vers") == version]
            if len(matches) > 1:
                raise ValueError(f"duplicate registry version for {name}@{version}")
            return matches[0] if matches else None
        except HTTPError as error:
            if error.code == 404:
                return None
            if error.code != 429 and not 500 <= error.code < 600:
                raise ValueError(f"registry lookup for {name} failed: HTTP {error.code}") from None
            if attempt == 5:
                raise ValueError(f"registry lookup for {name} exhausted retries: HTTP {error.code}") from None
            delay = retry_delay(error.headers, attempt)
        except (URLError, TimeoutError):
            if attempt == 5:
                raise ValueError(f"registry lookup for {name} exhausted network retries") from None
            delay = 2 ** attempt
        print(f"Retrying public registry lookup for {name} after {delay:g}s", flush=True)
        time.sleep(delay)
    raise ValueError("registry lookup did not complete")


def pending_packages(packages, directory):
    """Skip only identical, non-yanked versions; reject all byte conflicts."""
    pending = []
    for package in packages:
        name, version = package["name"], package["version"]
        archive = directory / f"{name}-{version}.crate"
        digest = hashlib.sha256(archive.read_bytes()).hexdigest()
        existing = registry_version(name, version)
        if existing is None:
            pending.append(package)
        elif existing.get("name") != name or existing.get("cksum") != digest:
            raise ValueError(f"{name}@{version} already exists with different package bytes; select a new version")
        elif existing.get("yanked") is not False:
            raise ValueError(f"{name}@{version} is yanked; select a new version")
        else:
            print(f"Already published with matching checksum: {name}@{version}", flush=True)
        # Keep public requests comfortably below registry API request limits too.
        time.sleep(1)
    return pending


def upload(arguments, token_file):
    """Load a token only for upload and redact it from Cargo's combined output."""
    token = (token_file.read_text().strip() if token_file else
             os.environ.get("CARGO_REGISTRY_TOKEN") or
             os.environ.get("CARGO_REGISTRIES_CRATES_IO_TOKEN", "").strip())
    if not token or re.search(r"\s", token):
        raise ValueError("upload requires one nonempty registry token via --token-file or CARGO_REGISTRY_TOKEN")
    environment = public_environment()
    environment["CARGO_REGISTRY_TOKEN"] = token
    with subprocess.Popen(
        arguments, cwd=ROOT, env=environment, text=True,
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
    ) as process:
        for line in process.stdout:
            print(line.replace(token, "[REDACTED]"), end="", flush=True)
        status = process.wait()
    if status:
        raise ValueError(f"cargo publish failed with exit status {status}; rerun from the same commit to resume")


def main(argv=None):
    """Verify the complete selected workspace before any credential is loaded."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dry-run", action="store_true", help="verify packages without uploading or reading a token")
    parser.add_argument("--token-file", type=Path, help="initial-publication token file; never passed in Cargo arguments")
    parser.add_argument("--version", help="require this exact workspace release version")
    args = parser.parse_args(argv)
    commit = clean_commit()
    run([sys.executable, "scripts/crate-resources.py", "check"])
    metadata = json.loads(run([
        "cargo", "metadata", "--format-version", "1", "--no-deps", "--locked",
    ], capture=True))
    packages, version = select_packages(metadata, args.version)
    selection = [argument for package in packages for argument in ("-p", package["name"])]
    print(f"Verifying {len(packages)} crates for {version} at {commit}", flush=True)
    # Cargo verifies the selected unpublished dependencies together. Never use
    # --no-verify or --allow-dirty, including on a resumed publication.
    run(["cargo", "package", "--locked", "--registry", "crates-io", *selection])
    directory = Path(metadata["target_directory"]) / "package"
    pending = pending_packages(packages, directory)
    if clean_commit() != commit:
        raise ValueError("Git HEAD changed during publication preparation")
    if args.dry_run:
        print(f"Dry run passed: {len(pending)} packages need publication; no credentials read or uploads attempted")
        return 0
    if not pending:
        print("All selected versions are already published with matching checksums")
        return 0
    selection = [argument for package in pending for argument in ("-p", package["name"])]
    upload(["cargo", "publish", "--locked", "--registry", "crates-io", *selection], args.token_file)
    remaining = pending_packages(packages, directory)
    if remaining:
        raise ValueError("publication returned before every selected version appeared in the public index; rerun to resume")
    print(f"Published {len(packages)} verified Sail crates at {version}")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f"publish-crates: {error}", file=sys.stderr)
        sys.exit(1)
