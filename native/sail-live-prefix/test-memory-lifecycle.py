"""Actual owned-process lifecycle controls; Linux kernel observations doubled.

These local tests do not qualify Linux procfs/pidfd behavior. A subsequent native
control must exercise genuine mm teardown and the held kernel identity guards.
"""
import argparse
import contextlib
import errno
import json
from pathlib import Path
import subprocess
import sys
import time
from unittest.mock import patch

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
import memory_lifecycle as lifecycle

CASES = ['already-terminal', 'exit-in-progress', 'live-ESRCH', 'eventual-exit-only',
         'foreign-starttime', 'foreign-pid', 'missing-proc', 'EIO-terminal',
         'permission-terminal', 'no-prior-RSS', 'nonzero-exit', 'terminal-timeout',
         'missing-pidfd', 'pidfd-not-terminal-after', 'foreign-reaped-pidfd']


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    args.out.mkdir(exist_ok=False)
    rows = []
    for case in CASES:
        child = subprocess.Popen([sys.executable, '-c',
            'import sys,time;sys.stdin.buffer.read(1);time.sleep(float(sys.argv[1]));sys.exit(int(sys.argv[2]))',
            '0.4' if case == 'terminal-timeout' else '0.005',
            '7' if case == 'nonzero-exit' else '0'], stdin=subprocess.PIPE)
        identity = dict(pid=child.pid, state='S', flags=0, starttime=17, pidfd_binding=[1, 2, 3])
        observed = dict(identity, flags=lifecycle.PF_EXITING)
        prior = 0 if case == 'no-prior-RSS' else 1
        errnum = {'EIO-terminal': errno.EIO, 'permission-terminal': errno.EACCES}.get(case, errno.ESRCH)
        error = OSError(errnum, 'retained controlled RSS observation')
        terminal_before = case in {'already-terminal', 'EIO-terminal', 'permission-terminal', 'nonzero-exit'}
        calls = []

        def terminal(fd, pid, reaped):
            assert pid == child.pid
            calls.append(fd)
            if case == 'missing-pidfd':
                raise ValueError('actual missing fixture descriptor')
            return terminal_before if len(calls) == 1 else case != 'pidfd-not-terminal-after'

        def task(pid):
            assert pid == child.pid
            if case == 'missing-proc':
                raise FileNotFoundError(errno.ENOENT, 'missing proc does not prove exit')
            return observed

        if case == 'foreign-starttime':
            observed['starttime'] += 1
        if case == 'foreign-pid':
            identity['pid'] += 1
        if case in {'live-ESRCH', 'eventual-exit-only'}:
            observed['flags'] = 0
        try:
            if case != 'live-ESRCH':
                child.stdin.write(b'x')
                child.stdin.flush()
            if terminal_before:
                child.wait(timeout=2)
            with contextlib.ExitStack() as stack:
                stack.enter_context(patch.object(lifecycle.os, 'uname', lambda: type('U', (), {'sysname': 'Linux'})()))
                stack.enter_context(patch.object(lifecycle, 'pidfd_binding', lambda _: [1, 8, 3] if case == 'foreign-reaped-pidfd' else [1, 2, 3]))
                stack.enter_context(patch.object(lifecycle, 'terminal_pidfd', terminal))
                stack.enter_context(patch.object(lifecycle, 'task_identity', task))
                result = lifecycle.terminal_observation(child, 123, identity, error, prior, time.monotonic() + 2)
            expected = case in {'already-terminal', 'exit-in-progress'}
            assert result['terminal_race'] is expected, (case, result)
            assert result['error'] and result['errno'] == errnum
            if expected:
                assert child.returncode == 0 and result['terminal_exit'] == 0
            if case == 'live-ESRCH':
                assert child.poll() is None
            rows.append(dict(case=case, passed=True, result=result))
            print('PASS', case)
        finally:
            if child.poll() is None:
                child.kill()
            child.wait(timeout=2)
            child.stdin.close()
    (args.out / 'results.json').write_text(json.dumps(dict(status='PASS', cases=len(rows), rows=rows,
        scope='Real owned Popen exit/status/wait; Linux procfs/pidfd observations explicitly doubled; native qualification pending.'), indent=2) + '\n')


if __name__ == '__main__':
    main()
