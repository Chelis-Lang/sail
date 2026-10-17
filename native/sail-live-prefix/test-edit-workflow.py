#!/usr/bin/env python3
"""Real listener, executable wrapper, caller and solver-fixture processes.

Linux pidfd/image/peer-credential APIs are explicit doubles on this Mac. Actual
wait4, pipes, Unix sockets, early version blocking, exit statuses, files and raw
streams run normally. This is not actual Verus/Z3/native/performance evidence.
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
from unittest.mock import patch

import edit_transport
import edit_workflow
import kernel_accounting
import live_prefix


SOLVER = '''import os,sys,time
batch=bytearray()
for line in sys.stdin.buffer:
 batch.extend(line)
 if line==b'(echo "<<DONE>>")\\n':
  if b':version' in batch:reply=b'(:version "4.16.0")\\n'
  elif b'check-sat' in batch:
   if b'SLOW' in batch:time.sleep(.4)
   reply=b'unsat\\n'
  else:reply=b'(:rlimit-count 1 :time 0.1)\\n'
  os.write(1,reply+b'<<DONE>>\\n');batch.clear()
if 'LATE7' in open('project.rs').read():
 os.write(2,b'actual controlled late exit7\\n');raise SystemExit(7)
'''
CALLER = '''import json,os,subprocess,sys,time
source=open('project.rs').read()
mode=os.environ['SAIL_TEST_EDIT_CASE']
if mode=='claim-ready-during-frontend' and source!='BASE\\n':time.sleep(.7)
if mode=='pre-first-record':
 try:subprocess.run(['/definitely/missing/research-wrapper'],capture_output=True)
 except FileNotFoundError as error:print(str(error))
 raise SystemExit(0)
environment=dict(os.environ)
if mode.startswith('tag-'):
 environment['SAIL_LIVE_PREFIX_SOLVER_CONTEXT']='v1:test:target' if mode!='tag-missing' else 'v1:test:other'
 helper=dict(environment)
 helper['SAIL_LIVE_PREFIX_SOLVER_CONTEXT']='v1:test:target' if mode=='tag-duplicate' else 'v1:test:helper'
 # Simulate a compiler-owned bucket tag, not a genuine Verus field. Reverse
 # accept order after the edit to prove selection does not follow ordinal0.
 def helper_session():
  data=b'(get-info :version)\\n(echo "<<DONE>>")\\n(assert true)\\n(check-sat)\\n(echo "<<DONE>>")\\n'
  r=subprocess.run([os.environ['VERUS_Z3_PATH'],'-smt2','-in'],env=helper,input=data,capture_output=True,timeout=8)
  assert r.returncode==0 and r.stdout.endswith(b'<<DONE>>\\n')
 if source=='BASE\\n':helper_session()
if mode=='context-miss' and source!='BASE\\n':environment['SAIL_TEST_CHANGED_CONTEXT']='edited-only'
arguments=['-bad'] if mode=='wrapper-wrong-args' else ['-smt2','-in']
child=subprocess.Popen([os.environ['VERUS_Z3_PATH'],*arguments],env=environment,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
version=b'(get-info :version)\\n(echo "<<DONE>>")\\n'
check=b'(assert false)\\n' if 'DIVERGE' in source else b'(assert true)\\n'
if mode=='unready' or mode.startswith('claim-'):check+=b';SLOW\\n'
check+=b'(check-sat)\\n(echo "<<DONE>>")\\n'
requests=[version,check,b'(get-info :all-statistics)\\n(echo "<<DONE>>")\\n']
if mode=='tag-long-prefix':requests.insert(2,check)
actual=[]
for request in requests:
 try:child.stdin.write(request);child.stdin.flush()
 except BrokenPipeError:break
 reply=bytearray()
 while not reply.endswith(b'<<DONE>>\\n'):
  block=os.read(child.stdout.fileno(),1)
  if not block:break
  reply.extend(block)
 actual.append(reply.hex())
 if not reply.endswith(b'<<DONE>>\\n'):break
try:child.stdin.close()
except BrokenPipeError:pass
code=child.wait(timeout=8);error=child.stderr.read()
child.stdout.close();child.stderr.close()
if mode.startswith('tag-') and source!='BASE\\n':helper_session()
extra=None
if mode=='claim-unready-no-promotion':
 second=subprocess.run([os.environ['VERUS_Z3_PATH'],'-smt2','-in'],env=environment,input=b''.join(requests),capture_output=True,timeout=8)
 extra={'exit':second.returncode,'stdout':second.stdout.hex(),'stderr':second.stderr.hex()}
print(json.dumps({'interposer_exit':code,'replies':actual,'stderr_hex':error.hex(),'extra':extra}))
# Deliberately ignore the real nonzero interposer exit, as the audited caller
# can. The independent broker/accounting gate must reject this compiler0.
raise SystemExit(0)
'''


@contextlib.contextmanager
def explicit_native_doubles(root):
    descriptors, seen = {}, set()
    lock = threading.Lock()
    def capture(pid, identity):
        first, second = os.open(os.devnull, os.O_RDONLY), os.open(os.devnull, os.O_RDONLY)
        descriptors[first] = pid
        return first, second, None
    def pidfd(pid):
        fd = os.open(os.devnull, os.O_RDONLY)
        descriptors[fd] = pid
        return fd
    def send(fd, sig):
        os.kill(descriptors[fd], sig)
    def peer(connection):
        # Explicit SO_PEERCRED double. The actual wrapper writes its real PID
        # before connect; controls are sequential and do not claim native
        # kernel peer authentication from this filesystem lookup.
        with lock:
            paths = list(root.rglob('*.start.json'))
            pending = [p for p in paths if str(p) not in seen]
            assert len(pending) == 1, pending
            seen.add(str(pending[0]))
            identity = edit_transport.read_record(pending[0])['identity']
            return dict(pid=identity['pid'], uid=os.getuid(), gid=os.getgid())
    with patch.object(live_prefix, 'capture_running_image', capture), patch.object(
            kernel_accounting, 'capture_running_image', capture), patch.object(
            live_prefix.Entry, '_image_ok', lambda _: True), patch.object(
            os, 'pidfd_open', pidfd, create=True), patch.object(
            signal, 'pidfd_send_signal', send, create=True), patch.object(
            edit_transport, 'peer', peer), patch.object(
            live_prefix.os, 'uname', lambda: os.uname_result(('Linux', '', '', '', 'x86_64'))):
        yield


rows = []
for case in ('same-prefix', 'unready', 'diverge-after-reply', 'late-exit7',
             'pre-first-record', 'context-miss', 'wrapper-wrong-args',
             'claim-ready-during-frontend', 'claim-unready-no-promotion',
             'spill-write-failure', 'spill-mutation', 'spill-unclosed',
             'tag-target', 'tag-long-prefix', 'tag-missing', 'tag-duplicate'):
    with tempfile.TemporaryDirectory(prefix='sle-', dir='/tmp') as td:
        root = Path(td)
        solver, caller, source = [root / name for name in ('solver', 'caller.py', 'project.rs')]
        solver.write_text('#!' + sys.executable + '\n' + SOLVER)
        solver.chmod(0o700)
        caller.write_text(CALLER)
        source.write_bytes(b'BASE\n')
        original = source.read_bytes()
        edited = {'same-prefix': b'BASE\n// edited body after first check\n',
                  'unready': b'BASE\n// fixed immediate edit\n',
                  'diverge-after-reply': b'DIVERGE\n', 'late-exit7': b'LATE7\n',
                  'pre-first-record': b'BASE\n', 'context-miss': b'BASE\n// context change\n',
                  'wrapper-wrong-args': b'BASE\n',
                  'claim-ready-during-frontend': b'BASE\n// actual caller frontend delay\n',
                  'claim-unready-no-promotion': b'BASE\n// two actual caller sessions\n',
                  'spill-write-failure': b'BASE\n// spill failure\n',
                  'spill-mutation': b'BASE\n// spill mutation\n',
                  'spill-unclosed': b'BASE\n// unclosed spill\n',
                  **{name: b'BASE\n// fixed historical target edit\n' for name in
                     ('tag-target', 'tag-long-prefix', 'tag-missing', 'tag-duplicate')}}[case]
        with patch.dict(os.environ, {'SAIL_TEST_EDIT_CASE': case}), explicit_native_doubles(root):
            workflow = edit_workflow.Workflow(root / 'out', solver,
                hashlib.sha256(solver.read_bytes()).hexdigest(),
                [sys.executable, '-B', str(caller)], root,
                edit_delay=0 if case == 'unready' or case.startswith('claim-') else .2, attempt_timeout=5,
                donor_context='v1:test:target' if case.startswith('tag-') else None,
                donor_prefix_batches=(3 if case=='tag-long-prefix' else 2) if case.startswith('tag-') else None,
                readiness_boundary='first-session-claim' if case.startswith('claim-') else 'edit-start')
            original_spill = workflow.spill_session
            spill_observations = []
            def observed_spill(directory, row):
                claims = row.get('claims', [])
                if case == 'spill-unclosed' and claims:
                    claims[0].closed = False
                    try:
                        original_spill(directory, row)
                    finally:
                        claims[0].closed = True
                    raise AssertionError('unclosed claim was accepted')
                original_spill(directory, row)
                assert 'input' not in row and 'output' not in row
                assert all(not c.input and not c.child_input for c in claims)
                spill_observations.append(row['ordinal'])
                if case == 'spill-mutation' and 'input' in row['_stored_streams']:
                    path = workflow.output / row['_stored_streams']['input']['path']
                    raw = path.read_bytes()
                    path.write_bytes(bytes([raw[0] ^ 1]) + raw[1:])
            workflow.spill_session = observed_spill
            real_open = Path.open
            class PartialWrite:
                def __init__(self, stream): self.stream = stream
                def __enter__(self): return self
                def __exit__(self, *args): return self.stream.__exit__(*args)
                def write(self, data):
                    self.stream.write(data[:3])
                    raise OSError('actual controlled partial save')
            def controlled_open(path, *args, **kwargs):
                actual = real_open(path, *args, **kwargs)
                if case == 'spill-write-failure' and path.name == 'session-0.input' and args == ('xb',):
                    return PartialWrite(actual)
                return actual
            with patch.object(Path, 'open', controlled_open):
                result = workflow.run(source, hashlib.sha256(original).hexdigest(), edited)
        assert source.read_bytes() == original and result['source_restored'], result
        actual = json.loads((root / 'out/result.json').read_bytes())
        assert edit_transport.canonical(actual) == edit_transport.canonical(result)
        assert result['qualified'] is result['selected'] is False
        assert result['application_exit'] == 1 and not result['application_acceptance']
        assert not list((root/'out').rglob('*.sock')), 'owned sockets must not survive archival'
        assert 'PENDING' in result['coverage']
        attempts = result['attempts']
        assert all(a['whole_attempt_wall_seconds'] >= a['compiler']['wall_seconds'] for a in attempts)
        if case in ('tag-missing', 'tag-duplicate'):
            assert result['driver_exit']==1 and len(attempts)==1 and result['errors']
            assert not result['preparation_claimed']
            assert 'unique frozen historical donor context required' in str(result['errors'])
        elif case in ('pre-first-record', 'wrapper-wrong-args') or case.startswith('spill-'):
            assert len(attempts) == 1 and result['driver_exit'] == 1
            assert attempts[0]['compiler']['exit'] == 0
            assert result['errors']
            if case.startswith('spill-'):
                assert attempts[0]['sessions']
                assert result['errors']
                if case == 'spill-write-failure':
                    assert (root / 'out/donor/session-0.input').stat().st_size == 3
                    assert (root / 'out/donor/session-0.unspilled-input').stat().st_size > 3
                if case == 'spill-mutation':
                    assert any('retained stream' in e for e in result['errors'])
            else:
                assert not attempts[0]['sessions']
            # Caller actually throws before record; no absence==success claim.
        else:
            assert not result['errors'], result
            assert attempts[0]['gate']['driver_exit'] == 0, result
            expected = 1 if case == 'late-exit7' else 0
            assert result['driver_exit'] == expected, result
            if case in ('same-prefix', 'diverge-after-reply', 'late-exit7'):
                assert result['preparation']['ready_at_edit_start'] is True, result
                assert result['preparation_claimed'], result
            if case.startswith('claim-'):
                observation = result['preparation']
                assert observation['boundary'] == 'first-session-claim'
                assert len(observation['claim_observations']) == 1
                expected_ready = case == 'claim-ready-during-frontend'
                assert observation['ready_at_session_claim'] == expected_ready, result
                assert result['preparation_claimed'] == expected_ready, result
                if not expected_ready:
                    # Preparation completes during the first fresh session;
                    # actual second session still cannot acquire it.
                    assert len(attempts[1]['sessions']) == 2
            if case == 'context-miss':
                assert result['preparation']['ready_at_edit_start'] is True
                assert not result['preparation_claimed']
            if case in ('tag-target', 'tag-long-prefix'):
                assert result['donor_selection']['ordinal']==1
                assert result['preparation_claimed']
                # The same frozen target moved to ordinal0 in edited/control.
                assert attempts[1]['sessions'][0]['context_sha256'] != attempts[0]['sessions'][0]['context_sha256']
            if case == 'unready':
                assert not result['preparation']['ready_at_edit_start']
                assert not result['preparation_claimed']
            if case in ('diverge-after-reply', 'late-exit7'):
                assert len(attempts) == 4
                assert attempts[1]['compiler']['exit'] == 0
                assert attempts[1]['gate']['driver_exit'] == 1
                assert attempts[2]['name'] == 'fresh-retry'
            else:
                assert len(attempts) == 3
            assert attempts[-1]['name'] == 'fresh-control'
            assert result['literal_compiler_streams_equal']
            assert result['owned_children']
            assert all(row['accounting'] is not None and not row['errors']
                       for row in result['owned_children'])
            if case == 'late-exit7':
                assert any(row['accounting']['exit'] == 7 for row in result['owned_children'])
                assert any(row['stderr']['bytes'] > 0 for row in result['owned_children'])
        rows.append(dict(name=case, passed=True, driver_exit=result['driver_exit'],
                         attempt_count=len(attempts), raw_bytes=result['raw_bytes'],
                         children=len(result.get('owned_children', []))))
# Real Unix accepts exercise the exact cumulative boundary with prior rows
# explicitly supplied as a counter fixture, not 512 claimed solver executions.
import socket
import time
with tempfile.TemporaryDirectory(prefix='sle-cap-', dir='/tmp') as td:
    def accepted_fixture(self, connection, row):
        row['admission_fixture'] = True
        connection.close()
    with patch.object(edit_transport.Broker, '_session', accepted_fixture):
        broker = edit_transport.Broker(Path(td) / 'broker', 'capacity', 'x' * 32,
                                      lambda *_: None, time.time() + 5)
        try:
            with broker.lock:
                broker.rows.extend(dict(ordinal=n, identity=None) for n in range(511))
            for expected_count in (512, 513):
                with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as connection:
                    connection.connect(str(broker.path))
                deadline = time.monotonic() + 2
                while len(broker.rows) < expected_count and time.monotonic() < deadline:
                    time.sleep(.001)
                assert len(broker.rows) == expected_count
            assert broker.rows[511].get('error') is None
            assert broker.rows[512]['error'] == 'session/concurrency budget refused'
        finally:
            outcome = broker.finish()
        assert outcome['socket_removed'] and not outcome['passed']
        assert broker.rows[511]['admission_fixture']
rows.append(dict(name='actual-accept-total512-next-refused', passed=True,
                 scope='511 counter-fixture rows + two real Unix accepts; no solver qualification'))
with tempfile.TemporaryDirectory(prefix='sle-active-', dir='/tmp') as td:
    release = threading.Event()
    broker = edit_transport.Broker(Path(td) / 'broker', 'active', 'x' * 32,
                                  lambda *_: None, time.time() + 5)
    try:
        for _ in range(16):
            worker = threading.Thread(target=release.wait)
            broker.workers.append(worker)
            worker.start()
        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as connection:
            connection.connect(str(broker.path))
        deadline = time.monotonic() + 2
        while not broker.rows and time.monotonic() < deadline:
            time.sleep(.001)
        assert broker.rows[0]['error'] == 'session/concurrency budget refused'
    finally:
        release.set()
        outcome = broker.finish()
    assert outcome['socket_removed'] and not outcome['passed']
rows.append(dict(name='actual-accept-active16-next-refused', passed=True,
                 scope='16 owned blocked thread fixtures + one real Unix accept; no solver qualification'))
with tempfile.TemporaryDirectory(prefix='sle-raw-', dir='/tmp') as td:
    root = Path(td)
    solver = root / 'solver'
    solver.write_text('#!' + sys.executable + '\nraise SystemExit(0)\n')
    solver.chmod(0o700)
    workflow = edit_workflow.Workflow(root / 'out', solver,
        hashlib.sha256(solver.read_bytes()).hexdigest(), [str(solver)], root,
        attempt_timeout=480)
    with explicit_native_doubles(root):
        pool = workflow.pool(dict(argv=[workflow.solver, '-smt2', '-in'], cwd=str(root.resolve()), environment={}))
        try:
            assert pool.run_timeout == 270 and workflow.timeout == 480
            assert pool.max_input_bytes == edit_workflow.MAX_BYTES == 16 * 1024**2
        finally:
            pool.close()
    rows.append(dict(name='compiler480-independent-solver270', passed=True))
    barrier = threading.Barrier(3)
    failures, captures = [], []
    def write_raw(name):
        barrier.wait(timeout=2)
        try:
            captures.append(workflow.raw(workflow.output, name, b'x' * 10))
        except ValueError as error:
            failures.append(str(error))
    with patch.object(edit_workflow, 'MAX_RAW', 15):
        workers = [threading.Thread(target=write_raw, args=(str(i),)) for i in range(2)]
        for worker in workers: worker.start()
        barrier.wait(timeout=2)
        for worker in workers: worker.join(timeout=2)
        assert all(not worker.is_alive() for worker in workers)
        assert len(captures) == len(failures) == 1 and workflow.copied == 10
    rows.append(dict(name='two-owned-thread-raw-reservation', passed=True, bytes_reserved=workflow.copied))
    workflow.copied = 0  # Explicit isolated accounting fixture, not a production refund.
    real_open = Path.open
    def partial_open(path, *args, **kwargs):
        actual = real_open(path, *args, **kwargs)
        return PartialWrite(actual) if path.name == 'partial' and args == ('xb',) else actual
    with patch.object(edit_workflow, 'MAX_RAW', 15), patch.object(Path, 'open', partial_open):
        try:
            workflow.raw(workflow.output, 'partial', b'x' * 10)
        except OSError:
            pass
        else:
            raise AssertionError('partial save unexpectedly passed')
        assert workflow.copied == 10
        try:
            workflow.raw(workflow.output, 'next', b'y' * 10)
        except ValueError:
            pass
        else:
            raise AssertionError('failed save reservation was refunded')
    assert (workflow.output / 'partial').stat().st_size == 3
    assert not (workflow.output / 'next').exists()
    rows.append(dict(name='actual-partial-save-reservation-not-refunded', passed=True, bytes_reserved=10))
with tempfile.TemporaryDirectory(prefix='sle-prefix-', dir='/tmp') as td:
    from edit_claim import StreamingClaim
    root = Path(td)
    solver = root / 'solver'
    solver.write_text('#!' + sys.executable + '\n' + SOLVER)
    solver.chmod(0o700)
    (root / 'project.rs').write_bytes(b'BASE\n')
    workflow = edit_workflow.Workflow(root / 'out', solver,
        hashlib.sha256(solver.read_bytes()).hexdigest(), [str(solver)], root,
        attempt_timeout=480)
    with explicit_native_doubles(root):
        pool = workflow.pool(dict(argv=[workflow.solver, '-smt2', '-in'], cwd=str(root.resolve()), environment={}))
        claim = None
        try:
            assert pool.seed(b'x' * (16 * 1024**2 + 1), b'') is None
            assert pool.last_attempt is None and not pool.entries
            prefix = b';' + b'x' * (2 * 1024**2 + 128) + b'\n(get-info :version)\n(echo "<<DONE>>")\n'
            check = b'(assert true)\n(check-sat)\n(echo "<<DONE>>")\n'
            key = pool.seed(prefix + check, b'(:version "4.16.0")\n<<DONE>>\nunsat\n<<DONE>>\n')
            assert key is not None and pool.wait_ready(key, 5), pool.last_refusal
            claim = StreamingClaim(pool, key)
            assert claim.snapshot_ready, claim.errors
            assert claim.batch(prefix)['action'] == 'prepared-reply'
            assert claim.batch(check)['action'] == 'prepared-reply'
            assert claim.finish(), claim.errors
            assert bytes(claim.input) == prefix + check
            assert claim.child_input.matches(prefix + check)
        finally:
            if claim is not None: claim.close()
            pool.close()
    rows.append(dict(name='actual-app-prefix-above2MiB-with16MiB-bound', passed=True,
                     prefix_bytes=len(prefix + check), next_over16MiB_refused_before_spawn=True))
print(json.dumps(dict(passed=True, scope=__doc__, rows=rows), indent=2))
