#!/usr/bin/env python3
"""Observe HELLO identity after an owned executable's bytes change at fixed mtime."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import socket
import struct
import subprocess
import tempfile
import time


def read_exact(stream, count):
    parts = []
    while count:
        data = stream.recv(count)
        if not data:
            raise RuntimeError("daemon closed a partial response")
        parts.append(data)
        count -= len(data)
    return b"".join(parts)


def hello(socket_path, solver):
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as stream:
        stream.settimeout(5)
        stream.connect(str(socket_path))
        request = b"\x01" + json.dumps({
            "solver_path": str(solver), "mode": "default",
        }).encode()
        stream.sendall(struct.pack(">I", len(request)) + request)
        size = struct.unpack(">I", read_exact(stream, 4))[0]
        if size > 65536:
            raise RuntimeError("unexpected HELLO response size")
        body = read_exact(stream, size)
        if len(body) < 5 or body[0] != 0:
            raise RuntimeError("HELLO refused: " + repr(body))
        size = struct.unpack(">I", body[1:5])[0]
        if size != len(body) - 5:
            raise RuntimeError("invalid HELLO payload size")
        return json.loads(body[5:])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sail", required=True, type=Path)
    args = parser.parse_args()
    sail = args.sail.resolve(strict=True)
    root = Path(tempfile.mkdtemp(prefix="sail-id-", dir="/tmp"))
    process = None
    try:
        old = b'#!/bin/sh\nprintf "Z3 version 4.16.0 - 64 bit\\n"\n# A\n'
        new = old[:-2] + b"B\n"
        solver = root / "solver"
        solver.write_bytes(old)
        solver.chmod(0o700)
        before = solver.stat()
        socket_path = root / "d.sock"
        env = {k: v for k, v in os.environ.items() if not k.startswith("SAIL_")}
        env["SAIL_CACHE_DIR"] = str(root / "cache")
        with (root / "daemon.out").open("wb") as out, (root / "daemon.err").open("wb") as err:
            process = subprocess.Popen([
                str(sail), "serve", "--socket", str(socket_path), "--cache",
                str(root / "results"), "--idle-timeout", "60",
            ], env=env, stdout=out, stderr=err)
            deadline = time.monotonic() + 10
            while not socket_path.exists():
                if process.poll() is not None or time.monotonic() >= deadline:
                    raise RuntimeError("owned daemon failed to become ready")
                time.sleep(0.03)
            first = hello(socket_path, solver)
            solver.write_bytes(new)
            os.utime(solver, ns=(before.st_atime_ns, before.st_mtime_ns))
            after = solver.stat()
            second = hello(socket_path, solver)
            expected_old = hashlib.sha256(old).hexdigest()
            expected_new = hashlib.sha256(new).hexdigest()
            if first["binary_sha256"] != expected_old:
                raise RuntimeError("initial identity was already incorrect")
            if (before.st_size, before.st_mtime_ns) != (after.st_size, after.st_mtime_ns):
                raise RuntimeError("fixture did not preserve length and mtime")
            matches = second["binary_sha256"] == expected_new
            print(json.dumps({
                "first_identity": first, "second_identity": second,
                "expected_new_sha256": expected_new,
                "same_path_length_mtime": True, "new_identity_matches_bytes": matches,
                "scope": "HELLO identity only; no solver verdict or proof exercised",
            }, indent=2))
            return 0 if matches else 1
    finally:
        if process is not None and process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=5)
        shutil.rmtree(root)


if __name__ == "__main__":
    raise SystemExit(main())
