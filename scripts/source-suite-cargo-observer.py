#!/usr/bin/env python3
"""Test-only Cargo stdout tee; never selected by ordinary builds or Sail.

An owned `cargo` copy of this script reads observer-config.json beside itself.
Output and exit status are forwarded even if bounded observation is refused;
controls must independently require complete sidecars. No solver is intercepted.
"""
import fcntl
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import uuid

MAX_BYTES = 16 * 1024**2
MAX_CALLS = 64


def main():
    config = json.loads(Path(__file__).with_name("observer-config.json").read_text())
    logs = Path(config["logs"])
    logs.mkdir(exist_ok=True)
    # Bound all concurrent observers together, independently of their callers.
    with (logs / "counter").open("a+b") as counter:
        fcntl.flock(counter, fcntl.LOCK_EX)
        counter.seek(0)
        number = int(counter.read() or b"0")
        counter.seek(0)
        counter.truncate()
        counter.write(str(min(number + 1, MAX_CALLS)).encode())
        counter.flush()
    recorded = number < MAX_CALLS
    if not recorded:
        (logs / "overflow").touch()
    key = uuid.uuid4().hex
    meta = logs / (key + ".json")
    output = logs / (key + ".stdout")
    record = dict(command=[config["real_cargo"], *os.sys.argv[1:]], cwd=os.getcwd(), complete=False)
    if recorded:
        meta.write_text(json.dumps(record) + "\n")
    # Match direct command signal handling; killed observers leave incomplete
    # initial sidecars rather than inventing successful observations.
    for number in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
        signal.signal(number, signal.SIG_DFL)
    log = output.open("wb") if recorded else None
    process = subprocess.Popen(record["command"], stdout=subprocess.PIPE)
    total = 0
    hasher = hashlib.sha256()
    observation_error = None
    try:
        while True:
            data = os.read(process.stdout.fileno(), 65536)
            if not data:
                break
            hasher.update(data)
            before = total
            total += len(data)
            if log is not None:
                try:
                    log.write(data[:max(0, MAX_BYTES - before)])
                except OSError as error:
                    observation_error = repr(error)
                    try:
                        log.close()
                    except OSError:
                        pass
                    log = None
            remaining = memoryview(data)
            while remaining:
                remaining = remaining[os.write(1, remaining):]
        code = process.wait()
    finally:
        if log is not None:
            try:
                log.close()
            except OSError as error:
                observation_error = repr(error)
        process.stdout.close()
    record.update(exit=code, total_bytes=total, stdout_sha256=hasher.hexdigest(),
                  complete=recorded and total <= MAX_BYTES and observation_error is None,
                  observation_error=observation_error)
    if recorded:
        temporary = meta.with_suffix(".tmp")
        temporary.write_text(json.dumps(record) + "\n")
        temporary.replace(meta)
    if code < 0:
        os.kill(os.getpid(), -code)
    raise SystemExit(code)


if __name__ == "__main__":
    main()
