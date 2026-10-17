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
CALLER = '''import json,os,subprocess,sys
source=open('project.rs').read()
mode=os.environ['SAIL_TEST_EDIT_CASE']
if mode=='pre-first-record':
 try:subprocess.run(['/definitely/missing/research-wrapper'],capture_output=True)
 except FileNotFoundError as error:print(str(error))
 raise SystemExit(0)
environment=dict(os.environ)
if mode=='context-miss' and source!='BASE\\n':environment['SAIL_TEST_CHANGED_CONTEXT']='edited-only'
arguments=['-bad'] if mode=='wrapper-wrong-args' else ['-smt2','-in']
child=subprocess.Popen([os.environ['VERUS_Z3_PATH'],*arguments],env=environment,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
version=b'(get-info :version)\\n(echo "<<DONE>>")\\n'
check=b'(assert false)\\n' if 'DIVERGE' in source else b'(assert true)\\n'
if mode=='unready':check+=b';SLOW\\n'
check+=b'(check-sat)\\n(echo "<<DONE>>")\\n'
requests=[version,check,b'(get-info :all-statistics)\\n(echo "<<DONE>>")\\n']
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
print(json.dumps({'interposer_exit':code,'replies':actual,'stderr_hex':error.hex()}))
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
             'pre-first-record', 'context-miss', 'wrapper-wrong-args'):
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
                  'wrapper-wrong-args': b'BASE\n'}[case]
        with patch.dict(os.environ, {'SAIL_TEST_EDIT_CASE': case}), explicit_native_doubles(root):
            workflow = edit_workflow.Workflow(root / 'out', solver,
                hashlib.sha256(solver.read_bytes()).hexdigest(),
                [sys.executable, '-B', str(caller)], root,
                edit_delay=0 if case == 'unready' else .2, attempt_timeout=5)
            result = workflow.run(source, hashlib.sha256(original).hexdigest(), edited)
        assert source.read_bytes() == original and result['source_restored'], result
        actual = json.loads((root / 'out/result.json').read_bytes())
        assert edit_transport.canonical(actual) == edit_transport.canonical(result)
        assert result['qualified'] is result['selected'] is False
        assert result['application_exit'] == 1 and not result['application_acceptance']
        assert 'PENDING' in result['coverage']
        attempts = result['attempts']
        if case in ('pre-first-record', 'wrapper-wrong-args'):
            assert len(attempts) == 1 and result['driver_exit'] == 1
            assert attempts[0]['compiler']['exit'] == 0
            assert result['errors']
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
            if case == 'context-miss':
                assert result['preparation']['ready_at_edit_start'] is True
                assert not result['preparation_claimed']
            if case == 'unready':
                assert not result['preparation']['ready_at_edit_start']
                assert not result['preparation_claimed']
            if case in ('diverge-after-reply', 'late-exit7'):
                assert len(attempts) == 3
                assert attempts[1]['compiler']['exit'] == 0
                assert attempts[1]['gate']['driver_exit'] == 1
                assert attempts[2]['name'] == 'fresh-retry'
            else:
                assert len(attempts) == 2
            assert result['owned_children']
            assert all(row['accounting'] is not None and not row['errors']
                       for row in result['owned_children'])
            if case == 'late-exit7':
                assert any(row['accounting']['exit'] == 7 for row in result['owned_children'])
                assert any(row['stderr']['bytes'] > 0 for row in result['owned_children'])
        rows.append(dict(name=case, passed=True, driver_exit=result['driver_exit'],
                         attempt_count=len(attempts), raw_bytes=result['raw_bytes'],
                         children=len(result.get('owned_children', []))))
print(json.dumps(dict(passed=True, scope=__doc__, rows=rows), indent=2))
