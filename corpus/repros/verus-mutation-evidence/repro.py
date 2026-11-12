#!/usr/bin/env python3
"""Retain bounded native Verus experiments; timeouts never become verdicts."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import signal
import subprocess
import time


class Interrupted(BaseException):
    """A requested interruption, propagated after recording and child cleanup."""

    def __init__(self, number):
        self.number = number


def interrupt(number, _frame):
    raise Interrupted(number)


def save(path, record):
    temporary = path.with_suffix('.temporary')
    temporary.write_text(json.dumps(record, indent=2) + '\n')
    temporary.replace(path)


def capture(command, directory, cap, metadata=None):
    record = dict(metadata or {}, command=command, cap_seconds=cap, status='starting', exit=None)
    process = None
    started = time.monotonic()
    try:
        save(directory / 'result.json', record)
        with (directory / 'stdout').open('wb') as stdout, (directory / 'stderr').open('wb') as stderr:
            process = subprocess.Popen(command, cwd=directory, stdin=subprocess.DEVNULL,
                                       stdout=stdout, stderr=stderr, start_new_session=True)
            record.update(status='running', process=process.pid)
            save(directory / 'result.json', record)
            try:
                process.wait(timeout=cap)
                record['status'] = 'finished'
            except subprocess.TimeoutExpired:
                record['status'] = 'timed-out'
    except Interrupted as error:
        record.update(status='interrupted', signal=error.number)
        raise
    except OSError as error:
        record.update(status='infrastructure-error', error=str(error))
    finally:
        previous = {number: signal.signal(number, signal.SIG_IGN)
                    for number in (signal.SIGINT, signal.SIGTERM)}
        try:
            if process is not None:
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                    record['group_cleanup'] = 'signalled'
                    if record['status'] == 'finished':
                        record.update(status='infrastructure-error',
                                      error='child group remained after command completion')
                except ProcessLookupError:
                    record['group_cleanup'] = 'absent'
                process.wait()
                record['exit'] = process.returncode
            record['seconds'] = time.monotonic() - started
            save(directory / 'result.json', record)
        finally:
            for number, handler in previous.items():
                signal.signal(number, handler)
    return record


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('install', type=Path)
    parser.add_argument('sources', nargs='+', type=Path)
    parser.add_argument('--out', required=True, type=Path)
    parser.add_argument('--runs', type=int, default=3)
    parser.add_argument('--cap', type=float, default=120)
    parser.add_argument('--verus-arg', action='append', default=[])
    args = parser.parse_args()
    if not 1 <= args.runs <= 20 or not 0 < args.cap <= 3600:
        parser.error('require 1..20 runs and a finite cap in (0,3600] seconds')
    install = args.install.resolve(strict=True)
    sources = [source.resolve(strict=True) for source in args.sources]
    if any(source.suffix != '.rs' or not source.is_file() for source in sources):
        parser.error('sources must name Rust files')
    if len({source.name for source in sources}) != len(sources):
        parser.error('source filenames must be distinct')
    args.out.mkdir(parents=True, exist_ok=False)
    out = args.out.resolve()
    identity = dict(platform=platform.platform(), machine=platform.machine(), install=str(install),
                    status='starting', attempts_started=0)
    failures = 0
    try:
        save(out / 'identity.json', identity)
        for binary in ('verus', 'z3'):
            directory = out / (binary + '-version')
            directory.mkdir()
            result = capture([str(install / binary), '--version'], directory, 30)
            identity[binary + '_version'] = result
            if result['status'] != 'finished' or result['exit'] != 0:
                identity['status'] = 'setup-failed'
                return 1
        for source in sources:
            original = source.read_bytes()
            for repetition in range(args.runs):
                directory = out / (source.stem + '-' + str(repetition))
                directory.mkdir()
                (directory / source.name).write_bytes(original)
                command = [str(install / 'verus'), '--crate-type=lib', *args.verus_arg, source.name]
                identity['attempts_started'] += 1
                save(out / 'identity.json', identity)
                result = capture(command, directory, args.cap,
                                 dict(source_sha256=hashlib.sha256(original).hexdigest()))
                failures += result['status'] == 'infrastructure-error'
                print(source.name, repetition, result['status'], result['exit'], flush=True)
        identity['status'] = 'completed'
        return int(failures > 0)
    except Interrupted as error:
        identity.update(status='interrupted', signal=error.number)
        return 128 + error.number
    finally:
        save(out / 'identity.json', identity)


if __name__ == '__main__':
    signal.signal(signal.SIGINT, interrupt)
    signal.signal(signal.SIGTERM, interrupt)
    raise SystemExit(main())
