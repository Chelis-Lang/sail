"""Actual Entry/owned-child drain and claim barrier; native guards doubled."""
import contextlib
import hashlib
import json
import os
from pathlib import Path
import sys
import tempfile
import time
from unittest.mock import patch

sys.dont_write_bytecode = True
import live_prefix

WORKER = r'''#!PYTHON
import os,sys,time
from pathlib import Path
sys.stdin.buffer.readline()
os.write(1,b'unsat\n(:time 1.234)\n')
while not Path('extra').exists():time.sleep(.001)
os.write(1,b'(error "partial')
while not Path('finish-extra').exists():time.sleep(.001)
os.write(1,b' response")\n')
sys.stdin.buffer.read()
'''


def until(predicate, seconds=3):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        if predicate():
            return
        time.sleep(.001)
    raise AssertionError('owned child/read barrier timed out')


def run():
    rows = []
    for experiment in [True, False]:
        with tempfile.TemporaryDirectory(prefix='sail readiness ') as td:
            root = Path(td)
            exe = root / 'worker'
            exe.write_text(WORKER.replace('PYTHON', sys.executable))
            exe.chmod(0o700)
            prefix = b'; original prefix\n'
            expected = b'unsat\n(:time 0.01)\n' if experiment else b'unsat\n(:time 1.234)\n'
            with contextlib.ExitStack() as stack:
                stack.enter_context(patch.object(os, 'uname', lambda: os.uname_result(('Linux', 'fixture', '', '', 'x86_64'))))
                stack.enter_context(patch.object(os, 'pidfd_open', lambda *_: None, create=True))
                stack.enter_context(patch.object(live_prefix.signal, 'pidfd_send_signal', lambda *_: None, create=True))
                stack.enter_context(patch.object(live_prefix, 'capture_running_image', lambda *_: (None, None, None)))
                stack.enter_context(patch.object(live_prefix.Entry, '_image_ok', lambda _: True))
                stack.enter_context(patch.object(live_prefix, 'resident_bytes', lambda _: (1024, 1024)))
                pool = live_prefix.OneUsePool(str(exe), hashlib.sha256(exe.read_bytes()).hexdigest(),
                    cwd=root, ttl=5, run_timeout=2, experiment_numeric_time_only=experiment)
                try:
                    key = pool.seed(prefix, expected)
                    assert pool.wait_ready(key, 2)
                    entry = pool.entries[key]
                    assert entry.ready
                    (root / 'extra').write_bytes(b'')
                    until(lambda: bytes(entry.stdout).endswith(b'(error "partial'))
                    with entry.changed:
                        assert not entry.ready
                        assert not entry.claimed
                    assert pool.consume(key, prefix + b'; original suffix\n') is None
                    if experiment:
                        # Partial parsing waits, but cannot consume the old readiness.
                        assert entry.failure is None
                        (root / 'finish-extra').write_bytes(b'')
                        until(lambda: entry.failure is not None)
                        assert entry.failure == 'unexpected prefix output'
                    else:
                        # The unchanged exact mode refuses additional bytes at once.
                        assert entry.failure == 'unexpected prefix output'
                    assert not entry.claimed
                    rows.append({'experiment': experiment, 'passed': True,
                        'actual_drained_prefix': bytes(entry.stdout).decode(),
                        'failure': entry.failure, 'claimed': entry.claimed})
                finally:
                    pool.close()
                assert entry.process.poll() is not None
    print(json.dumps({'status': 'PASS', 'cases': len(rows), 'rows': rows,
        'scope': 'Actual Entry thread/drain/consume with owned child and real pipe barriers. Native image and RSS are fixture doubles; no solver/native qualification.'}, indent=2))


if __name__ == '__main__':
    run()
