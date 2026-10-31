#!/usr/bin/env python3
"""Diagnostic syscall accounting for one real command on a native Linux host.

Use --out DIRECTORY -- COMMAND... . strace changes timing; its elapsed time
never supplies a benchmark sample. No syscall arguments or source bytes are
captured. Keep raw reports outside the repository.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time


ROW = re.compile(r"^\s*([\d.]+)\s+([\d.]+)\s+(\d+)\s+(\d+)(?:\s+(\d+))?\s+(\w+)\s*$")


def parse_summary(text):
    records = {}
    for line in text.splitlines():
        match = ROW.match(line)
        if match and match[6] != "total":
            if match[6] in records:
                raise ValueError("duplicate syscall summary")
            records[match[6]] = {"calls": int(match[4]), "errors": int(match[5] or 0),
                                 "traced_cpu_seconds": float(match[2])}
        elif line.strip() and not (line.lstrip().startswith(("% time", "---"))
                                   or line.rstrip().endswith("total")):
            raise ValueError("unrecognized strace summary line: " + line)
    if not records:
        raise ValueError("empty syscall summary")
    return records


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=lambda value: Path(value).resolve(), required=True)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    command = args.command[1:] if args.command[:1] == ["--"] else args.command
    if not command:
        parser.error("a command is required")
    args.out.mkdir(parents=True, exist_ok=False)
    summary = args.out / "syscalls.txt"
    argv = ["strace", "-f", "-c", "-qq", "-e",
            "trace=%file,read,readv,pread64,getdents64,close", "-o", str(summary), "--", *command]
    record = {"schema_version": 1, "command": command, "cwd": str(Path.cwd()),
              "diagnostic_only": True, "started_unix": time.time()}
    env = dict(os.environ, LC_ALL="C")
    result = subprocess.run(argv, env=env, check=False)
    record["exit"] = result.returncode
    record["elapsed_seconds_including_tracer"] = time.time() - record["started_unix"]
    try:
        record["syscalls"] = parse_summary(summary.read_text())
        record["summary_sha256"] = hashlib.sha256(summary.read_bytes()).hexdigest()
    except (OSError, ValueError) as error:
        record["refusal"] = str(error)
    (args.out / "work.json").write_text(json.dumps(record, indent=2) + "\n")
    return (result.returncode if result.returncode >= 0 else 128 - result.returncode) or (
        2 if "refusal" in record else 0)


if __name__ == "__main__":
    sys.exit(main())
