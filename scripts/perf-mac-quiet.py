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


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    try:
        before = snapshot()
        started = time.monotonic()
        time.sleep(1)
        result = assess(before, snapshot(), time.monotonic()-started)
        args.output.write_text(json.dumps(result, indent=2)+'\n')
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
