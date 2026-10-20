#!/usr/bin/env python3
"""ci/timeout.py matches GNU timeout's exit statuses (and GNU itself where installed)."""
import shutil
import subprocess
import sys
from pathlib import Path

WRAPPER = [sys.executable, str(Path(__file__).with_name('timeout.py'))]
CASES = [
    (['5', 'sh', '-c', 'exit 3'], 3),
    (['0.2', 'sleep', '5'], 124),
    # Killed by KILL: a shell reports 137; Python sees the signal.
    (['--signal=TERM', '--kill-after=0.3s', '0.2s', 'sh', '-c', 'trap "" TERM; sleep 5'], -9),
    (['-s', 'INT', '0.2', 'sleep', '5'], 124),
    (['5', 'sh', '-c', 'kill -TERM $$'], -15),
    (['--bogus', '1', 'true'], 125),
    (['1', '/nonexistent/command'], 127),
]


def main():
    implementations = [('ci/timeout.py', WRAPPER)]
    if shutil.which('timeout'):
        implementations.append(('GNU timeout', ['timeout']))
    for name, prefix in implementations:
        for args, expected in CASES:
            status = subprocess.run(prefix + args, capture_output=True, timeout=30).returncode
            assert status == expected, f'{name} {args}: exit {status}, expected {expected}'
    print('timeout: exit statuses match GNU timeout (' + ', '.join(n for n, _ in implementations) + ')')


if __name__ == '__main__':
    main()
