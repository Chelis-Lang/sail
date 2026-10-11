#!/usr/bin/env python3
"""Guest remote-debug preflight on the owned standalone fixture only."""
import hashlib
import json
import os
from pathlib import Path
import select
import signal
import subprocess
import time

BASE = Path('/diagnostic/remote')
BASE.mkdir(exist_ok=True)
FIXTURE = Path('/diagnostic/fixture')
PORT = 31337
def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def starttime(pid):
    return int(Path(f'/proc/{pid}/stat').read_text().rsplit(') ', 1)[1].split()[19])
record = {'version': 1, 'scope': 'Standalone guest remote-debug preflight; no proof actor',
          'fixture_sha256': digest(FIXTURE), 'status': 'running',
          'diagnostic_only_env': {'ROSETTA_DEBUGSERVER_PORT': str(PORT)}}
proc = debug = None
raw = bytearray()
try:
    env = dict(os.environ)
    env['ROSETTA_DEBUGSERVER_PORT'] = str(PORT)
    proc = subprocess.Popen([str(FIXTURE)], env=env, stdin=subprocess.PIPE,
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True)
    identity = starttime(proc.pid)
    record['actor'] = {'pid': proc.pid, 'pgid': proc.pid, 'starttime_ticks': identity}
    command = ['gdb', '-nx', '-batch', str(FIXTURE),
               '-ex', 'set pagination off', '-ex', 'set confirm off',
               '-ex', 'set print frame-arguments none', '-ex', 'set remotetimeout 2',
               '-ex', 'set architecture i386:x86-64',
               '-ex', f'target remote 127.0.0.1:{PORT}',
               '-ex', 'break diagnostic_pipe_wait_marker', '-ex', 'continue',
               '-ex', 'thread apply all bt 12', '-ex', 'detach', '-ex', 'quit']
    record['debugger_command'] = command
    began = time.monotonic()
    deadline = began + 5
    debug = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                             start_new_session=True)
    try:
        while True:
            remaining = deadline-time.monotonic()
            if remaining <= 0 or not select.select([debug.stdout], [], [], remaining)[0]:
                raise TimeoutError('Remote debugger exceeded 5-second preflight budget')
            chunk = os.read(debug.stdout.fileno(), 4096)
            if not chunk:
                break
            if len(raw)+len(chunk) > 256*1024:
                raise RuntimeError('Remote debugger output exceeded 256 KiB')
            raw.extend(chunk)
        debug.wait(timeout=max(0.001, deadline-time.monotonic()))
    finally:
        if debug.poll() is None:
            os.killpg(debug.pid, signal.SIGKILL)
            debug.wait(timeout=2)
        (BASE/'gdb.stdout').write_bytes(raw)
        record['debugger_exit_code'] = debug.returncode
        record['debugger_elapsed_seconds'] = time.monotonic()-began
    if starttime(proc.pid) != identity:
        raise RuntimeError('Owned fixture identity changed')
    os.kill(proc.pid, signal.SIGCONT)
    proc.stdin.write(b'x')
    proc.stdin.flush()
    proc.stdin.close()
    proc.stdin = None
    stdout, stderr = proc.communicate(timeout=2)
    (BASE/'fixture.stdout').write_bytes(stdout)
    (BASE/'fixture.stderr').write_bytes(stderr)
    text = raw.decode(errors='replace')
    record['fixture_exit_code'] = proc.returncode
    record['checks'] = {'debugger_exit_zero': debug.returncode == 0,
                        'both_known_guest_frames': all(name in text for name in
                          ('diagnostic_condition_wait_marker', 'diagnostic_pipe_wait_marker')),
                        'detach_reported': 'detached' in text.lower(),
                        'fixture_normal_exit_after_detach': proc.returncode == 0}
    record['status'] = 'PASS' if all(record['checks'].values()) else 'FAIL'
except BaseException as error:
    record.update(status='FAIL', error_type=type(error).__name__, error=str(error))
finally:
    if debug is not None and debug.poll() is None:
        os.killpg(debug.pid, signal.SIGKILL)
        debug.wait(timeout=2)
    if proc is not None and proc.poll() is None:
        try:
            if starttime(proc.pid) == record['actor']['starttime_ticks']:
                os.killpg(proc.pid, signal.SIGCONT)
                os.killpg(proc.pid, signal.SIGKILL)
        except FileNotFoundError:
            pass
        proc.wait(timeout=2)
    record['actor_closed'] = proc is None or proc.poll() is not None
    record['raw_sha256'] = {p.name: digest(p) for p in BASE.iterdir() if p.is_file() and p.name != 'preflight.json'}
    (BASE/'preflight.json').write_text(json.dumps(record, indent=2)+'\n')
    print(json.dumps(record, indent=2))
raise SystemExit(0 if record['status'] == 'PASS' else 1)
