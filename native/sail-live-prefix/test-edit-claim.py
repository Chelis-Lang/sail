#!/usr/bin/env python3
"""Actual owned interactive children; Linux identity/pidfd APIs explicitly doubled.

No genuine solver, interposer IPC, compiler workflow or native qualification.
Existing kernel-owner implementation supplies actual sole-wait4 accounting.
"""
import contextlib
import hashlib
import json
import os
from pathlib import Path
import signal
import sys
import tempfile
import threading
import time
import tracemalloc
from unittest.mock import patch

import kernel_accounting
import live_prefix
from edit_claim import StreamingClaim, DirectClaim

VERSION = b'(get-info :version)\n(echo "<<DONE>>")\n'
CHECK = b'(assert true)\n(check-sat)\n(echo "<<DONE>>")\n'
STATS = b'(get-info :all-statistics)\n(echo "<<DONE>>")\n'
VR = b'(:version "4.16.0")\n<<DONE>>\n'
CR = b'unsat\n<<DONE>>\n'
SR = b'(:rlimit-count 1 :time 0.1)\n<<DONE>>\n'
rows = []


@contextlib.contextmanager
def native_doubles():
    descriptors = {}
    def capture(pid, identity):
        first = os.open(os.devnull, os.O_RDONLY)
        second = os.open(os.devnull, os.O_RDONLY)
        descriptors[first] = pid
        return first, second, None
    def send(fd, sig):
        os.kill(descriptors[fd], sig)
    with patch.object(live_prefix, 'capture_running_image', capture), patch.object(
            kernel_accounting, 'capture_running_image', capture), patch.object(
            live_prefix.Entry, '_image_ok', lambda _: True), patch.object(
            os, 'pidfd_open', lambda *_: None, create=True), patch.object(
            signal, 'pidfd_send_signal', send, create=True), patch.object(
            live_prefix.os, 'uname', lambda: os.uname_result(('Linux', '', '', '', 'x86_64'))):
        yield


WORKER = '''import os,sys,time
if os.environ.get('SAIL_TEST_SLOW_PREPARATION')=='1':time.sleep(1)
batch=bytearray()
for line in sys.stdin.buffer:
 batch.extend(line)
 if line==b'(echo "<<DONE>>")\\n':
  if b'(get-info :version)' in batch:reply=b'(:version "4.16.0")\\n'
  elif b'(check-sat)' in batch:reply=b'unsat\\n'
  else:
   reply=b'(:rlimit-count 1 :time 0.1)\\n'
   if os.environ.get('SAIL_TEST_STDERR_STATS')=='1':os.write(2,b'actual controlled solver stderr\\n')
  os.write(1,reply+b'<<DONE>>\\n');batch.clear()
if os.environ.get('SAIL_TEST_HANG_EOF')=='1':time.sleep(2)
'''


for case in ('success', 'unready', 'diverge-before', 'diverge-after', 'duplicate-claim',
             'ipc-after-reply', 'child-after-reply', 'missing-accounting',
             'stderr-after-reply', 'timeout-after-eof', 'direct-success', 'direct-ipc',
             'direct-parser-retention',
             'pool-busy-at-edit-start', 'entry-busy-at-edit-start',
             'direct-loader-before', 'direct-loader-after',
             'prepared-loader-before', 'prepared-loader-after', 'prepared-loader-idle',
             'prepared-loader-idle-expired'):
    with tempfile.TemporaryDirectory() as td, native_doubles(), contextlib.ExitStack() as local:
        worker = Path(td) / 'solver-fixture'
        worker.write_text('#!' + sys.executable + '\n' + WORKER)
        worker.chmod(0o700)
        digest = hashlib.sha256(worker.read_bytes()).hexdigest()
        env = dict(os.environ)
        if case == 'unready':
            env['SAIL_TEST_SLOW_PREPARATION'] = '1'
        if case == 'stderr-after-reply':
            env['SAIL_TEST_STDERR_STATS'] = '1'
        if case == 'timeout-after-eof':
            env['SAIL_TEST_HANG_EOF'] = '1'
        loader_target = None
        if '-loader-' in case:
            if case.startswith('prepared-'):
                from edit_loader import RustupLoader
                local.enter_context(patch.object(RustupLoader, 'observe'))
            home = Path(td).resolve() / 'rustup'
            lib = home / 'toolchains' / 'fixture' / 'lib'
            lib.mkdir(parents=True)
            loader_target = Path(td).resolve() / 'target'
            (loader_target / 'debug' / 'deps').mkdir(parents=True)
            env.update(RUSTUP_HOME=str(home), LD_LIBRARY_PATH=str(lib))
        pool = live_prefix.OneUsePool(str(worker), digest, max_bytes=1024**3,
                                     ttl=10, run_timeout=.3 if case == 'timeout-after-eof' else 5, environment=env,
                                     experiment_numeric_time_only=True,
                                     experiment_kernel_accounting_owner=True,
                                     experiment_rustup_loader_target=loader_target)
        claim = None
        entry = None
        try:
            if case.startswith('direct-'):
                claim = DirectClaim(pool)
                assert claim.snapshot_ready, claim.errors
                entry = claim.entry
                assert entry.prefix == b''
                if case.startswith('direct-loader-'):
                    assert dict(pool.environment) == env  # No loader stripping.
                    observed = []
                    def observe(pid, fd):
                        observed.append((pid, fd))
                        if case == 'direct-loader-before' or len(observed) > 1:
                            raise ValueError('controlled actual caller library refusal')
                    entry.loader.observe = observe
                    first = claim.batch(VERSION)
                    if case == 'direct-loader-before':
                        assert first['action'] == 'fresh-before-reply' and bytes(entry.stdout) == VR
                    else:
                        assert first == {'action': 'original-child-reply', 'reply': VR}
                        assert claim.batch(CHECK)['action'] == 'abort-whole-attempt'
                    assert observed and not claim.completed
                    continue
                assert claim.batch(VERSION) == {'action': 'original-child-reply', 'reply': VR}
                if case == 'direct-ipc':
                    assert claim.abort('controlled direct IPC disconnect')['action'] == 'abort-whole-attempt'
                else:
                    check = CHECK
                    if case == 'direct-parser-retention':
                        check = b';' + b'x' * 65536 + b'\n' + CHECK
                        original_write = claim._write
                        retained = []
                        def observed_write(raw):
                            snapshot = tracemalloc.take_snapshot()
                            retained.append(sum(statistic.size for statistic in snapshot.statistics('filename')
                                if statistic.traceback[0].filename.endswith('/edit_stream.py')))
                            assert retained[-1] < 4096, retained
                            return original_write(raw)
                        claim._write = observed_write
                        tracemalloc.start()
                    try:
                        assert claim.batch(check) == {'action': 'original-child-reply', 'reply': CR}
                    finally:
                        if case == 'direct-parser-retention':
                            tracemalloc.stop()
                            claim._write = original_write
                    if case == 'direct-parser-retention':
                        assert len(retained) == 1
                        rows.append(dict(name='parsed-input-live-bytes-before-owned-write', passed=True,
                                         input_bytes=len(check), retained_parser_bytes=retained[0]))
                    assert claim.batch(STATS) == {'action': 'original-child-reply', 'reply': SR}
                    assert claim.finish(), claim.errors
                    assert bytes(claim.input) == bytes(claim.child_input) == VERSION + check + STATS
                continue
            key = pool.seed(VERSION + CHECK, VR + CR)
            assert key is not None, pool.last_refusal
            entry = pool.entries[key]
            if case != 'unready':
                # Fixture synchronization BEFORE edit start; the adapter never
                # waits for preparation, and the unready case does not do this.
                assert pool.wait_ready(key, 3), entry.failure
            if case.startswith('prepared-loader-idle'):
                observed = []
                entry.loader.observe = lambda pid, fd: observed.append((pid, fd))
                # Actual idle owned child and monitor threads: no new input or
                # output can justify repeating the completed-prefix hash.
                time.sleep(.12)
                assert observed == [], observed
                if case.endswith('-expired'):
                    with entry.changed:
                        entry.deadline = time.monotonic() + .05
                    until = time.monotonic() + 2
                    while not entry.failure and time.monotonic() < until:
                        time.sleep(.01)
                    assert entry.failure == 'preparation/idle/run deadline exceeded', entry.failure
            if case.endswith('-busy-at-edit-start'):
                entered, release = threading.Event(), threading.Event()
                lock = pool.lock if case.startswith('pool-') else entry.changed
                def hold():
                    with lock:
                        entered.set()
                        assert release.wait(2)
                holder = threading.Thread(target=hold)
                holder.start()
                assert entered.wait(1)
                start = time.monotonic()
                try:
                    claim = StreamingClaim(pool, key)
                    assert time.monotonic() - start < .2
                    assert not claim.snapshot_ready and claim.entry is None
                    assert case in claim.errors
                    assert entry.process.poll() is None
                finally:
                    release.set()
                    holder.join(timeout=2)
                assert not holder.is_alive()
            else:
                claim = StreamingClaim(pool, key)
            if case.endswith('-busy-at-edit-start'):
                assert not entry.claimed
            elif case in ('unready', 'prepared-loader-idle-expired'):
                assert not claim.snapshot_ready
                assert claim.batch(VERSION)['action'] == 'fresh-before-reply'
            else:
                assert claim.snapshot_ready, claim.errors
                if case in ('prepared-loader-before', 'prepared-loader-after'):
                    observed = []
                    def observe(pid, fd):
                        observed.append((pid, fd))
                        if case == 'prepared-loader-before' or len(observed) > 1:
                            raise ValueError('controlled prepared caller library refusal')
                    entry.loader.observe = observe
                    first = claim.batch(VERSION)
                    if case == 'prepared-loader-before':
                        assert first['action'] == 'fresh-before-reply'
                    else:
                        assert first == {'action': 'prepared-reply', 'reply': VR}
                        assert claim.batch(CHECK)['action'] == 'abort-whole-attempt'
                    assert observed and not claim.completed
                elif case == 'diverge-before':
                    assert claim.batch(VERSION.replace(b':version', b':name'))['action'] == 'fresh-before-reply'
                else:
                    assert claim.batch(VERSION) == {'action': 'prepared-reply', 'reply': VR}
                    if case == 'diverge-after':
                        changed = CHECK.replace(b'assert true', b'assert false')
                        assert claim.batch(changed)['action'] == 'abort-whole-attempt'
                        claim.close()
                        # A distinct WHOLE fresh attempt receives all actual
                        # edited bytes. No emitted prefix is spliced/dropped.
                        fresh = kernel_accounting.fresh(str(worker), digest,
                                                        VERSION + changed, 3, 1024**3)
                        assert fresh['kernel_resource_gate_pass'] and fresh['stdout'] == VR + CR
                        rows.append({'name': 'distinct-whole-fresh-retry',
                                     'input_bytes': fresh['input_bytes_written'],
                                     'actual_exit': fresh['exit'],
                                     'cpu': kernel_accounting.cpu(fresh['kernel_accounting'])})
                    elif case == 'ipc-after-reply':
                        assert claim.abort('controlled IPC disconnect')['action'] == 'abort-whole-attempt'
                    elif case == 'child-after-reply':
                        entry._kill_owned()
                        entry.process.wait(timeout=2)
                        assert claim.batch(CHECK)['action'] == 'abort-whole-attempt'
                    else:
                        if case == 'duplicate-claim':
                            second = StreamingClaim(pool, key)
                            assert not second.snapshot_ready and second.entry is None
                            second.close()
                            assert entry.process.poll() is None
                        assert claim.batch(CHECK) == {'action': 'prepared-reply', 'reply': CR}
                        stats = claim.batch(STATS)
                        if case == 'stderr-after-reply':
                            # Pipe scheduling may expose stderr before or
                            # after the complete stdout batch; neither can
                            # make the completed attempt qualify.
                            assert stats['action'] in ('original-child-reply', 'abort-whole-attempt')
                            assert not claim.finish()
                            assert b'actual controlled solver stderr\n' == bytes(entry.stderr)
                        elif case == 'timeout-after-eof':
                            assert stats == {'action': 'original-child-reply', 'reply': SR}
                            assert not claim.finish() and claim.errors
                        elif case == 'missing-accounting':
                            assert stats == {'action': 'original-child-reply', 'reply': SR}
                            with patch.object(entry.process, 'accounting', side_effect=ChildProcessError('controlled missing rusage')):
                                assert not claim.finish()
                            assert claim.errors and not claim.completed
                        else:
                            assert stats == {'action': 'original-child-reply', 'reply': SR}
                            assert claim.finish(), claim.errors
                            assert entry.kernel_accounting['exit'] == 0
                            assert entry.kernel_accounting['accounting']['peak_rss_bytes'] > 0
                            assert bytes(claim.input) == bytes(claim.child_input) == VERSION + CHECK + STATS
                            if case == 'prepared-loader-idle':
                                # One claim validation plus a fresh validation
                                # before each of two prepared and one live replies.
                                assert len(observed) == 4, observed
            if case not in ('success', 'duplicate-claim', 'prepared-loader-idle'):
                assert not claim.completed
        finally:
            if claim is not None:
                claim.close()
            pool.close()
            if case.startswith('direct-') and entry is not None:
                record = entry.process.accounting()
                assert entry.kernel_closed and not any(t.is_alive() for t in entry.threads)
                assert all(s.closed for s in (entry.process.stdin, entry.process.stdout, entry.process.stderr))
                assert entry.pidfd is None and entry.image_fd is None
                assert claim.completed == (case in ('direct-success', 'direct-parser-retention'))
                rows.append({'name': case, 'passed': True, 'actual_exit': record['exit'],
                             'complete_cpu': record['accounting']['user_seconds'] + record['accounting']['system_seconds'],
                             'retained_stdout_bytes': len(entry.stdout)})
        record = entry.process.accounting()
        assert entry.kernel_closed and not any(t.is_alive() for t in entry.threads)
        assert all(s.closed for s in (entry.process.stdin, entry.process.stdout, entry.process.stderr))
        assert entry.pidfd is None and entry.image_fd is None
        rows.append({'name': case, 'passed': True, 'actual_exit': record['exit'],
                     'peak_rss_bytes': record['accounting']['peak_rss_bytes'],
                     'complete_cpu': record['accounting']['user_seconds'] + record['accounting']['system_seconds'],
                     'retained_stdout_bytes': len(entry.stdout), 'retained_stderr_bytes': len(entry.stderr)})

print(json.dumps({'passed': True, 'rows': rows, 'scope': __doc__}, indent=2))
