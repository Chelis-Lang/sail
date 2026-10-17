#!/usr/bin/env python3
"""Actual IPC/caller/grandchild lifecycle; Linux image/pidfd APIs explicitly doubled.

The caller deliberately ignores a real interposer exit1 after final replies.
No genuine Verus/Z3, native or application performance qualification is supplied.
"""
import contextlib
import hashlib
import json
import os
from pathlib import Path
import signal
import socket
import struct
import subprocess
import sys
import tempfile
import threading
from unittest.mock import patch

import kernel_accounting
import live_prefix
from edit_claim import DirectClaim
from edit_wire import serve, send, attempt_status

HERE = Path(__file__).resolve().parent
BINDING = {'fixture_only': True}
TOKEN = 'controlled-private-connection'
INPUT = [b'(get-info :version)\n(echo "<<DONE>>")\n',
         b'(assert true)\n(check-sat)\n(echo "<<DONE>>")\n']
OUTPUT = [b'(:version "4.16.0")\n<<DONE>>\n', b'unsat\n<<DONE>>\n']
WORKER = '''import os,sys
batch=bytearray()
for line in sys.stdin.buffer:
 batch.extend(line)
 if line==b'(echo "<<DONE>>")\\n':
  os.write(1,(b'(:version "4.16.0")\\n' if b':version' in batch else b'unsat\\n')+b'<<DONE>>\\n');batch.clear()
raise SystemExit(int(os.environ.get('SAIL_TEST_EXIT_AFTER_EOF','0')))
'''
CLIENT = '''import socket,sys,json
sys.path.insert(0,sys.argv[2])
from edit_wire import interpose
connection=socket.socket(fileno=int(sys.argv[1]))
raise SystemExit(interpose(connection,{'fixture_only':True},'controlled-private-connection',sys.stdin.buffer,sys.stdout.buffer))
'''
CALLER = '''import os,subprocess,sys,json
fd=int(sys.argv[2]);child=subprocess.Popen([sys.executable,'-B',sys.argv[1],str(fd),sys.argv[3]],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,pass_fds=(fd,));os.close(fd)
requests=json.loads(sys.argv[5]);responses=json.loads(sys.argv[6]);actual=[]
for request,response in zip(requests,responses):
 child.stdin.write(bytes.fromhex(request));child.stdin.flush();raw=bytearray()
 while not raw.endswith(b'<<DONE>>\\n'):
  block=os.read(child.stdout.fileno(),1)
  if not block:break
  raw.extend(block)
 actual.append(raw.hex())
 if raw!=bytes.fromhex(response):break
child.stdin.close();code=child.wait(timeout=5);stderr=child.stderr.read();child.stdout.close();child.stderr.close()
open(sys.argv[4],'w').write(json.dumps({'actual_interposer_exit':code,'responses':actual,'stderr_hex':stderr.hex()}))
# Deliberately mirrors the audited Ok(ExitStatus) handling: status is retained
# but ignored. The outer driver must not borrow this compiler exit0.
raise SystemExit(0)
'''


@contextlib.contextmanager
def native_doubles():
    descriptors = {}
    def capture(pid, _):
        first, second = os.open(os.devnull, os.O_RDONLY), os.open(os.devnull, os.O_RDONLY)
        descriptors[first] = pid
        return first, second, None
    def kill(fd, sig):
        os.kill(descriptors[fd], sig)
    with patch.object(live_prefix, 'capture_running_image', capture), patch.object(
            kernel_accounting, 'capture_running_image', capture), patch.object(
            live_prefix.Entry, '_image_ok', lambda _: True), patch.object(
            os, 'pidfd_open', lambda *_: None, create=True), patch.object(
            signal, 'pidfd_send_signal', kill, create=True), patch.object(
            live_prefix.os, 'uname', lambda: os.uname_result(('Linux', '', '', '', 'x86_64'))):
        yield


def pool_for(worker, exit_code):
    return live_prefix.OneUsePool(str(worker), hashlib.sha256(worker.read_bytes()).hexdigest(),
                                 max_bytes=1024**3, ttl=10, run_timeout=5,
                                 environment=dict(os.environ, SAIL_TEST_EXIT_AFTER_EOF=str(exit_code)),
                                 experiment_numeric_time_only=True,
                                 experiment_kernel_accounting_owner=True)


rows = []
with tempfile.TemporaryDirectory() as td, native_doubles():
    root = Path(td)
    worker, client, caller = [root / name for name in ('solver', 'client.py', 'caller.py')]
    worker.write_text('#!' + sys.executable + '\n' + WORKER)
    worker.chmod(0o700)
    client.write_text(CLIENT)
    caller.write_text(CALLER)
    def actual_attempt(label, exit_code):
        pool = pool_for(worker, exit_code)
        claim = DirectClaim(pool)
        assert claim.snapshot_ready, claim.errors
        left, right = socket.socketpair()
        left.settimeout(6)
        returned = []
        thread = threading.Thread(target=lambda: returned.append(serve(left, claim, BINDING, TOKEN)))
        thread.start()
        process = None
        try:
            process = subprocess.Popen([sys.executable, '-B', str(caller), str(client), str(right.fileno()),
                                        str(HERE), str(root / (label + '.json')),
                                        json.dumps([x.hex() for x in INPUT]),
                                        json.dumps([x.hex() for x in OUTPUT])],
                                       pass_fds=(right.fileno(),), stdout=subprocess.PIPE,
                                       stderr=subprocess.PIPE, start_new_session=True)
            right.close()
            stdout, stderr = process.communicate(timeout=10)
            assert stdout == stderr == b''
            thread.join(timeout=2)
            assert not thread.is_alive() and len(returned) == 1
            result = returned[0]
            record = json.loads((root / (label + '.json')).read_text())
            assert record['responses'] == [x.hex() for x in OUTPUT]
            assert result['input'] == b''.join(INPUT) and result['output'] == b''.join(OUTPUT)
            assert claim.entry.kernel_accounting['exit'] == exit_code
            assert process.returncode == 0
            admission = attempt_status(process.returncode, [result])
            assert admission['driver_exit'] == (0 if exit_code == 0 else 1)
            assert record['actual_interposer_exit'] == (0 if exit_code == 0 else 1)
            rows.append({'name': label, 'actual_compiler_exit': process.returncode,
                         'actual_interposer_exit': record['actual_interposer_exit'],
                         'actual_backend_exit': claim.entry.kernel_accounting['exit'],
                         'driver_admission': admission,
                         'input_sha256': hashlib.sha256(result['input']).hexdigest(),
                         'output_sha256': hashlib.sha256(result['output']).hexdigest(),
                         'error': result['error']})
            return admission
        finally:
            if process is not None and process.poll() is None:
                os.killpg(process.pid, signal.SIGKILL)
                process.communicate(timeout=2)
            right.close()
            if thread.is_alive():
                left.close()
                claim.abort('owned test cleanup')
                thread.join(timeout=2)
            claim.close()
            pool.close()
    failed = actual_attempt('late-backend7-caller-ignores-status', 7)
    assert failed['driver_exit'] == 1 and failed['whole_fresh_retry_required']
    retry = actual_attempt('separate-whole-fresh-retry', 0)
    assert retry['driver_exit'] == 0 and not retry['qualified']
    assert rows[0]['input_sha256'] == rows[1]['input_sha256']

    # A real partial IPC write must retain available SMT bytes before refusal.
    pool = pool_for(worker, 0)
    claim = DirectClaim(pool)
    left, right = socket.socketpair()
    left.settimeout(3)
    returned = []
    thread = threading.Thread(target=lambda: returned.append(serve(left, claim, BINDING, TOKEN)))
    thread.start()
    send(right, ord('H'), json.dumps({'binding': BINDING, 'token': TOKEN}).encode())
    right.sendall(b'I' + struct.pack('!I', 100) + b'(assert')
    right.close()
    thread.join(timeout=5)
    assert not thread.is_alive() and len(returned) == 1
    assert returned[0]['input'] == b'(assert' and not returned[0]['passed']
    rows.append({'name': 'actual-partial-frame-retained', 'retained_input_hex': returned[0]['input'].hex(),
                 'error': returned[0]['error']})
    claim.close()
    pool.close()

for compiler, sessions in [(0, []), (0, [None]), (7, [{'passed': True, 'complete': True}]),
                           (0, [{'passed': True, 'complete': False}])]:
    assert attempt_status(compiler, sessions)['driver_exit'] == 1
rows.append({'name': 'empty-foreign-incomplete-nonzero-driver-refusals', 'passed': True})
print(json.dumps({'passed': True, 'rows': rows, 'scope': __doc__}, indent=2))
