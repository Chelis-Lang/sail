#!/usr/bin/env python3
"""Retain bounded, read-only context evidence for hosted cache acceptance.

Only runtime identity paths/digests and hashes of named noncredential selector
values are recorded. No source environment, token, proof receipt or cache data
is copied, and this collector never authorizes verification or reuse.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import stat

MAX_BYTES = 2 * 1024 * 1024
SELECTORS = ("PATH", "HOME", "CARGO_HOME", "RUSTUP_HOME", "RUSTUP_TOOLCHAIN",
             "RUSTC", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "RUSTFLAGS",
             "CARGO_ENCODED_RUSTFLAGS", "CARGO_BUILD_TARGET", "CARGO_TARGET_DIR")

def read_json(path):
    fd = os.open(path, os.O_RDONLY | os.O_NONBLOCK | os.O_NOFOLLOW | os.O_CLOEXEC)
    with os.fdopen(fd, "rb") as source:
        metadata = os.fstat(source.fileno())
        if not stat.S_ISREG(metadata.st_mode) or metadata.st_size > MAX_BYTES:
            raise ValueError("context evidence is not an ordinary bounded JSON file")
        raw = source.read(MAX_BYTES + 1)
        if len(raw) != metadata.st_size:
            raise ValueError("context evidence changed or exceeded its bound")
        return json.loads(raw), hashlib.sha256(raw).hexdigest()

def digest(value):
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--result", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--cache-root", type=Path)
    args = parser.parse_args()
    root = args.cache_root or Path(os.environ.get("SAIL_CACHE_DIR") or
        str(Path(os.environ.get("XDG_CACHE_HOME") or Path.home()/".cache")/"sail"))
    record = {"version": 1, "available": False,
              "scope": "diagnostic context identity; no proof or reuse authority"}
    try:
        result, result_sha = read_json(args.result)
        context = result["context_sha256"]
        if not digest(context):
            raise ValueError("invalid result context identity")
        identity, link_sha = read_json(root / "runtime-identities" / (context + ".link"))
        if not digest(identity):
            raise ValueError("invalid runtime identity link")
        files, files_sha = read_json(root / "runtime-identities" / (identity + ".json"))
        if not isinstance(files, list):
            raise ValueError("invalid runtime identity inventory")
        selectors = {}
        for name in SELECTORS:
            raw = os.environb.get(name.encode())
            selectors[name] = {"present": raw is not None,
                "sha256": hashlib.sha256(raw).hexdigest() if raw is not None else None}
        record.update(available=True, run_id=result["run_id"], context_sha256=context,
            input_revision=result["input_revision"], execution=result["execution"],
            result_file_sha256=result_sha, identity_sha256=identity,
            link_file_sha256=link_sha, identity_file_sha256=files_sha,
            runtime_identities=files, caller_selector_hashes=selectors,
            runner_image=os.environ.get("ImageVersion"))
    except (OSError, ValueError, KeyError, TypeError) as error:
        record["unavailable_reason"] = str(error)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(record, indent=2) + "\n")
    print(json.dumps({"available":record["available"],"output":str(args.out)}), flush=True)

if __name__ == "__main__":
    main()
