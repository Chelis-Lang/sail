#!/usr/bin/env python3
"""Transport only authenticated temporal receipts between trusted Sail installations."""
import hashlib
import io
import os
from pathlib import Path
import re
import stat
import struct
import subprocess
import sys
import tempfile

MAGIC = b"Sail temporal CI receipts\x00\x01"
MAX_BUNDLE = 64 * 1024 * 1024
MAX_RECEIPT = 16 * 1024 * 1024 + 4096
MAX_ENTRIES = 4096
NAMESPACE = "sail-temporal-receipts-v1"


def owned(info):
    return info.st_uid == os.geteuid() and info.st_mode & 0o022 == 0


def directory(path, create=False):
    if create:
        path.mkdir(mode=0o700, exist_ok=True)
    info = path.lstat()
    if not stat.S_ISDIR(info.st_mode) or not owned(info):
        raise ValueError("unsafe temporal receipt directory")


def receipt_tree(root, create=False):
    # root is <configured cache>/runtime/temporal-v1/v2. Check each
    # component before following it, matching the runtime's local trust model.
    cache = root.parents[2]
    if create:
        cache.mkdir(mode=0o700, parents=True, exist_ok=True)
    for path in (cache, root.parents[1], root.parent, root):
        directory(path, create)


def regular(path, maximum):
    """Bound the opened file, including a file replaced during selection."""
    with os.fdopen(os.open(path, os.O_NOFOLLOW | os.O_NONBLOCK), "rb") as stream:
        info = os.fstat(stream.fileno())
        if not stat.S_ISREG(info.st_mode) or not owned(info) or info.st_size > maximum:
            raise ValueError("unsuitable receipt file")
        data = stream.read(maximum + 1)
        if len(data) > maximum or len(data) != info.st_size:
            raise ValueError("receipt changed during read")
        return data


def pack(root, repository):
    data = io.BytesIO(MAGIC + hashlib.sha256(repository.encode()).digest())
    data.seek(0, io.SEEK_END)
    count = 0
    try:
        receipt_tree(root)
    except FileNotFoundError:
        return None
    for shard in sorted(root.iterdir()):
        if not re.fullmatch("[0-9a-f]{2}", shard.name):
            continue
        directory(shard)
        for path in shard.iterdir():
            if not re.fullmatch(shard.name + "[0-9a-f]{62}", path.name):
                continue
            try:
                blob = regular(path, MAX_RECEIPT)
            except (OSError, ValueError):
                continue
            if not blob or data.tell() + 36 + len(blob) > MAX_BUNDLE:
                continue
            data.write(bytes.fromhex(path.name))
            data.write(struct.pack(">I", len(blob)))
            data.write(blob)
            count += 1
            if count == MAX_ENTRIES:
                return data.getvalue()
    return data.getvalue() if count else None


def records(blob, repository):
    prefix = MAGIC + hashlib.sha256(repository.encode()).digest()
    if len(blob) > MAX_BUNDLE or not blob.startswith(prefix):
        raise ValueError("incompatible bundle")
    offset = len(prefix)
    entries = {}
    while offset < len(blob):
        if offset + 36 > len(blob) or len(entries) >= MAX_ENTRIES:
            raise ValueError("invalid receipt header")
        key = blob[offset:offset + 32].hex()
        length = struct.unpack_from(">I", blob, offset + 32)[0]
        offset += 36
        if key in entries or not 0 < length <= MAX_RECEIPT or offset + length > len(blob):
            raise ValueError("invalid receipt length or key")
        entries[key] = memoryview(blob)[offset:offset + length]
        offset += length
    return entries


def public_key():
    fields = os.environ.get("SAIL_RECEIPT_PUBLIC_KEY", "").split()
    if len(fields) < 2 or fields[0] != "ssh-ed25519" or not re.fullmatch("[A-Za-z0-9+/]+=*", fields[1]):
        raise ValueError("expected a trusted Ed25519 OpenSSH public key")
    return " ".join(fields[:2])


def ssh(args, data):
    result = subprocess.run(["ssh-keygen", "-Y", *args], input=data, capture_output=True,
                            timeout=15, env={"PATH": os.defpath, "LC_ALL": "C"})
    if result.returncode:
        raise ValueError("receipt signature check or signing failed")
    return result.stdout


def verify(blob, signature, trusted):
    with tempfile.TemporaryDirectory() as temp:
        signers = Path(temp) / "signers"
        signers.write_text("sail " + trusted + "\n")
        sig = Path(temp) / "signature"
        sig.write_bytes(signature)
        ssh(["verify", "-f", str(signers), "-I", "sail", "-n", NAMESPACE, "-s", str(sig)], blob)


def publish(path, data):
    # NamedTemporaryFile creates mode 0600 independently of the runner's umask.
    with tempfile.NamedTemporaryFile(dir=path.parent, delete=False) as stream:
        try:
            stream.write(data)
            stream.flush()
            os.replace(stream.name, path)
        finally:
            Path(stream.name).unlink(missing_ok=True)


def export_bundle(root, stage, repository, trusted, secret):
    directory(stage)
    blob = pack(root, repository)
    if not blob:
        return False
    with tempfile.TemporaryDirectory() as temp:
        path = Path(temp) / "key"
        with open(path, "w", opener=lambda p, flags: os.open(p, flags, 0o600)) as stream:
            stream.write(secret.rstrip() + "\n")
        signature = ssh(["sign", "-f", str(path), "-n", NAMESPACE, "-"], blob)
    verify(blob, signature, trusted)
    publish(stage / "bundle", blob)
    publish(stage / "signature", signature)
    return True


def import_bundle(root, stage, repository, trusted):
    directory(stage)
    if not (stage / "bundle").exists():
        return 0
    blob = regular(stage / "bundle", MAX_BUNDLE)
    verify(blob, regular(stage / "signature", 4096), trusted)
    entries = records(blob, repository)
    receipt_tree(root, create=True)
    for key, data in entries.items():
        shard = root / key[:2]
        directory(shard, create=True)
        publish(shard / key, data)
    return len(entries)


def output(name, value):
    with open(os.environ["GITHUB_OUTPUT"], "a") as stream:
        print(f"{name}={value}", file=stream)


def main():
    command = sys.argv[1]
    repository = os.environ["GITHUB_REPOSITORY_ID"]
    if not repository.isdecimal():
        raise ValueError("invalid repository identity")
    trusted = public_key()
    stage = Path(os.environ["RUNNER_TEMP"]) / "sail-temporal-receipts-v1"
    cache = Path(os.environ.get("SAIL_CACHE_DIR") or
                 Path(os.environ.get("XDG_CACHE_HOME") or Path.home() / ".cache") / "sail")
    root = cache / "runtime" / "temporal-v1" / "v2"
    if command == "setup":
        mode = os.environ.get("SAIL_RECEIPT_MODE")
        if mode not in {"save", "restore"}:
            raise ValueError("mode must be save or restore")
        directory(stage, create=True)
        if not stage.is_absolute():
            raise ValueError("invalid runner staging directory")
        for name in ("bundle", "signature"):
            (stage / name).unlink(missing_ok=True)
        output("path", stage)
        output("scope", hashlib.sha256((repository + "/" + trusted).encode()).hexdigest())
    elif command == "import":
        try:
            count = import_bundle(root, stage, repository, trusted)
        except (OSError, ValueError, subprocess.TimeoutExpired) as error:
            print(f"Temporal cache miss ({error})")
        else:
            print(f"Imported {count} authenticated temporal receipts")
    elif command == "export":
        saved = export_bundle(root, stage, repository, trusted, os.environ["SAIL_RECEIPT_SECRET"])
        output("present", str(saved).lower())
    else:
        raise ValueError("unknown receipt command")


if __name__ == "__main__":
    main()
