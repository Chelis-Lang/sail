#!/usr/bin/env python3
"""Download exact original tool archives and prepare one unselected runtime."""
from pathlib import Path
import argparse
import hashlib
import http.client
import importlib.util
import json
import os
import stat
import sys
import tempfile
import urllib.parse
import urllib.request

spec = importlib.util.spec_from_file_location("sail_kani_install", Path(__file__).with_name("kani-install.py"))
preparation = importlib.util.module_from_spec(spec)
spec.loader.exec_module(preparation)


class HttpsRedirectHandler(urllib.request.HTTPRedirectHandler):
    """Reject every transport downgrade before urllib follows its target."""

    def redirect_request(self, request, response, code, message, headers, new_url):
        if urllib.parse.urlsplit(new_url).scheme != "https":
            response.close()
            raise ValueError("original tool redirect requires HTTPS")
        return super().redirect_request(request, response, code, message, headers, new_url)


def fetch(url, path, size, digest):
    """Bound and authenticate original bytes; never overwrite an existing file."""
    if type(size) is not int or not 0 < size <= preparation.rust.MAX_ARCHIVE:
        raise ValueError("invalid original tool download bound")
    preparation.bundle.check_hash(digest)
    if urllib.parse.urlsplit(url).scheme != "https":
        raise ValueError("original tool download requires HTTPS")
    with urllib.request.build_opener(HttpsRedirectHandler()).open(url, timeout=60) as response:
        if urllib.parse.urlsplit(response.geturl()).scheme != "https":
            raise ValueError("original tool redirect requires HTTPS")
        remaining = size
        value = hashlib.sha256()
        with path.open("xb") as output:
            while remaining:
                block = response.read(min(128 * 1024, remaining))
                if not block:
                    raise ValueError("truncated original tool download")
                output.write(block)
                value.update(block)
                remaining -= len(block)
            if response.read(1):
                raise ValueError("excessive original tool download")
        if value.hexdigest() != digest:
            raise ValueError("original tool download digest differs")


def download(bundle_pin_path, rust_pin_path, destination):
    """Prepare a private sibling and publish only the complete original payloads."""
    if not destination.is_absolute() or not destination.name or os.path.lexists(destination):
        raise ValueError("original runtime needs an absolute new destination")
    parent = destination.parent.resolve(strict=True)
    if not parent.is_dir() or stat.S_IMODE(parent.stat().st_mode) & 0o077:
        raise ValueError("original runtime parent must be private")
    bundle = preparation.bundle.load_pin(bundle_pin_path)
    with tempfile.TemporaryDirectory(prefix=".sail-kani-download-", dir=parent) as temporary:
        archives = Path(temporary)
        manifest = archives / "channel-rust-nightly.toml"
        # The original manifest identity precedes every caller-supplied pin URL.
        fetch(f"https://static.rust-lang.org/dist/{preparation.rust.DATE}/channel-rust-nightly.toml",
              manifest, 975_638, preparation.rust.MANIFEST_SHA256)
        rust = preparation.rust.load_pin(rust_pin_path, manifest)
        for row in rust["components"]:
            fetch(row["url"], archives / row["url"].rsplit("/", 1)[-1], row["archive_bytes"], row["archive_sha256"])
        original_bundle = archives / "bundle.tar.gz"
        fetch(bundle["url"], original_bundle, bundle["archive_bytes"], bundle["archive_sha256"])
        return preparation.install(original_bundle, bundle, archives, rust, parent / destination.name)


def main():
    """Prepare the pinned Linux runtime; installing does not select a backend."""
    parser = argparse.ArgumentParser(description=__doc__)
    base = Path(__file__).resolve().parents[1] / "engine/kani"
    parser.add_argument("--bundle-pin", type=Path, default=base / "bundle-linux.json")
    parser.add_argument("--rust-pin", type=Path, default=base / "toolchain-linux.json")
    parser.add_argument("--destination", type=Path, required=True)
    args = parser.parse_args()
    if sys.platform != "linux" or os.uname().machine != "x86_64":
        raise ValueError("original runtime setup supports native Linux x86-64 only")
    print(json.dumps(download(args.bundle_pin, args.rust_pin, args.destination), sort_keys=True))


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, KeyError, TypeError, RecursionError, http.client.HTTPException, preparation.tarfile.TarError) as error:
        print(f"error[SAIL6305]: original Kani runtime setup: {error}", file=sys.stderr)
        raise SystemExit(2) from None
