#!/usr/bin/env python3
"""Refuse Mac measurement when a fresh one-second CPU observation is busy."""
import argparse
import json
from pathlib import Path
import subprocess
import sys
import time

MAX_CORES = .10


def cpu_seconds(value):
    days, separator, clock = value.partition('-')
    seconds = int(days) * 86400 if separator else 0
    fields = (clock if separator else days).split(':')
    for index, field in enumerate(reversed(fields)):
        seconds += float(field) * 60 ** index
    return seconds


def parse(text):
    rows = {}
    for line in text.splitlines():
        fields = line.split(None, 7)
        if len(fields) != 8:
            raise ValueError('incomplete process inventory')
        pid = int(fields[0])
        if pid in rows:
            raise ValueError('duplicate process identity')
        rows[pid] = (' '.join(fields[1:6]), cpu_seconds(fields[6]), fields[7])
    if not rows:
        raise ValueError('empty process inventory')
    return rows


def snapshot():
    return parse(subprocess.check_output(
        ['ps', '-A', '-o', 'pid=,lstart=,time=,comm='], text=True, timeout=5))


def assess(before, after, seconds):
    if not 0 < seconds < 10:
        raise ValueError('invalid observation interval')
    rows = []
    for pid, (start, used, command) in after.items():
        previous = before.get(pid)
        baseline = previous[1] if previous and previous[0] == start else 0
        delta = used - baseline
        if delta < 0:
            raise ValueError('process CPU counter moved backwards')
        rows.append(dict(pid=pid, command=command, cores=delta/seconds))
    total = sum(row['cores'] for row in rows)
    return dict(schema=1, interval_seconds=seconds, total_cores=total,
                maximum_cores=MAX_CORES, quiet=total <= MAX_CORES,
                processes=sorted(rows, key=lambda row: row['cores'], reverse=True))


def measure_once():
    before = snapshot()
    started = time.monotonic()
    time.sleep(1)
    return assess(before, snapshot(), time.monotonic()-started)


def wait_for_quiet(wait_seconds, *, probe=None, record=None, clock=None, sleeper=None):
    """Wait only during setup; retain every window and require a fresh pass.

    The zero default preserves the original single-check refusal. No workload
    runs here. A busy reading may consume setup time, never a measured sample;
    exhausting the bounded wait still refuses the run.
    """
    if not 0 <= wait_seconds <= 120:
        raise ValueError('setup wait must be finite and between zero and 120 seconds')
    probe = probe or measure_once
    clock = clock or time.monotonic
    sleeper = sleeper or time.sleep
    deadline = clock() + wait_seconds
    observations = []
    while True:
        result = probe()  # unreadable/invalid inventories propagate as refusals
        observations.append(result)
        summary = dict(result, setup_wait_seconds=wait_seconds,
                       observations=list(observations))
        if record is not None:
            record(summary)
        if result['quiet']:
            return summary
        remaining = deadline - clock()
        if remaining <= 0:
            return summary
        sleeper(min(5, remaining))
        if clock() >= deadline:
            return summary


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--wait-seconds', type=float, default=0,
                        help='bounded setup wait for the unchanged fresh CPU ceiling (max 120)')
    args = parser.parse_args()
    try:
        result = wait_for_quiet(
            args.wait_seconds,
            record=lambda value: args.output.write_text(json.dumps(value, indent=2)+'\n'))
        if not result['quiet']:
            print('Mac measurement refused: background CPU uses '
                  f"{result['total_cores']:.3f} cores (limit {MAX_CORES:.2f}); "
                  f'see {args.output}', file=sys.stderr)
            return 2
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(f'Mac measurement refused: cannot establish quiet CPU: {error}', file=sys.stderr)
        return 2
    return 0


if __name__ == '__main__':
    sys.exit(main())
