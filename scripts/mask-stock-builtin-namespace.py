#!/usr/bin/env python3
"""Mask only ADR 0112's public home prefix for the scanner's default checks.

The caller restricts this to three exact regular runtime metadata positions.
Explicit caller tokens always scan the original file. Memory and path lookahead
are bounded; no query, diagnostic or released artifact is rewritten.
"""
import os
from pathlib import Path
import stat
import sys

NAMESPACES = {
    "macos": b"/Users/runner/work/verus/verus/source/builtin",
    "linux": b"/home/runner/work/verus/verus/source/builtin",
}
PATH_BYTES = frozenset(b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_./-\\") | frozenset(range(128, 256))
CHUNK = 1024 * 1024
LOOKAHEAD = 8192
MAX_PATH = 4096


def masked(data, stop, namespace):
    result = bytearray(data)
    cursor = 0
    home = b"/Users/" if namespace.startswith(b"/Users/") else b"/home/"
    while True:
        at = data.find(namespace, cursor)
        if at < 0 or at >= stop:
            break
        cursor = at + len(namespace)
        end = cursor
        while end < len(data) and data[end] in PATH_BYTES and end - cursor < MAX_PATH:
            end += 1
        tail = data[cursor:end]
        # A following identifier is a prefix lookalike. Only the exact directory
        # or canonical descendants qualify; traversal/dot/duplicate separators do not.
        if tail and not tail.startswith(b"/"):
            continue
        if len(tail) >= MAX_PATH:
            continue
        if b"\\" in tail:
            continue
        if tail not in (b"", b"/") and any(part in (b"", b".", b"..") for part in tail[1:].split(b"/")):
            continue
        result[at:at + len(home)] = b"\0" * len(home)
    return result


def regular(path, flags):
    fd = os.open(path, flags | os.O_NONBLOCK | os.O_NOFOLLOW)
    try:
        if not stat.S_ISREG(os.fstat(fd).st_mode):
            raise ValueError(f"not a regular metadata file: {path}")
        return fd
    except BaseException:
        os.close(fd)
        raise


def main():
    if len(sys.argv) != 4 or sys.argv[1] not in NAMESPACES:
        raise ValueError("expected macos|linux INPUT EXISTING_PRIVATE_OUTPUT")
    namespace = NAMESPACES[sys.argv[1]]
    with os.fdopen(regular(Path(sys.argv[2]), os.O_RDONLY), "rb") as source:
        with os.fdopen(regular(Path(sys.argv[3]), os.O_WRONLY | os.O_TRUNC), "wb") as output:
            pending = b""
            while chunk := source.read(CHUNK):
                pending += chunk
                stop = max(0, len(pending) - LOOKAHEAD)
                transformed = masked(pending, stop, namespace)
                output.write(transformed[:stop])
                pending = bytes(transformed[stop:])
            output.write(masked(pending, len(pending), namespace))


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError) as error:
        print(f"scan: cannot inspect canonical builtin metadata: {error}", file=sys.stderr)
        sys.exit(2)
