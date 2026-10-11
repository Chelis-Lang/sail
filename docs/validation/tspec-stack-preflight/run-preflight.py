#!/usr/bin/env python3
"""Owned known-symbol preflight only; no Verus/proof/corpus actor."""
import hashlib
import json
import os
from pathlib import Path
import select
import signal
import subprocess
import time

BASE = Path('/diagnostic')

def starttime(pid):
    return int(Path(f'/proc/{pid}/stat').read_text().rsplit(') ', 1)[1].split()[19])

def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

record = {'version': 1, 'scope': 'Owned known-symbol x86 debugger preflight only; no compiler or proof actor',
          'fixture_sha256': digest(BASE/'fixture.c'), 'status': 'running'}
proc = None
try:
    compiled = subprocess.run(['cc', '-g', '-O0', '-fno-omit-frame-pointer', '-pthread',
                               str(BASE/'fixture.c'), '-o', str(BASE/'fixture')],
                              capture_output=True, timeout=30)
    (BASE/'compile.stdout').write_bytes(compiled.stdout)
    (BASE/'compile.stderr').write_bytes(compiled.stderr)
    record['compile_exit_code'] = compiled.returncode
    if compiled.returncode:
        raise RuntimeError('Standalone diagnostic fixture compilation failed')
    record['fixture_binary_sha256'] = digest(BASE/'fixture')
    proc = subprocess.Popen([str(BASE/'fixture')], stdin=subprocess.PIPE,
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True)
    identity = starttime(proc.pid)
    record['actor'] = {'pid': proc.pid, 'pgid': proc.pid, 'starttime_ticks': identity}
    if not select.select([proc.stdout], [], [], 2)[0] or proc.stdout.readline() != b'READY\n':
        raise RuntimeError('Owned fixture did not become ready')
    command = ['gdb', '-nx', '-batch', '-ex', 'set pagination off', '-ex', 'set confirm off',
               '-ex', 'set print frame-arguments none', '-ex', 'set print thread-events off',
               '-ex', f'attach {proc.pid}', '-ex', 'thread apply all bt 12', '-ex', 'detach', '-ex', 'quit']
    began = time.monotonic()
    debug = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                             start_new_session=True)
    raw = bytearray()
    deadline = began + 5
    record['debugger_command'] = command
    try:
        while True:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise TimeoutError('Debugger exceeded original preflight 5-second budget')
            available, _, _ = select.select([debug.stdout], [], [], remaining)
            if not available:
                raise TimeoutError('Debugger read exceeded preflight budget')
            chunk = os.read(debug.stdout.fileno(), 4096)
            if not chunk:
                break
            if len(raw) + len(chunk) > 256 * 1024:
                raise RuntimeError('Debugger output exceeded 256 KiB')
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
    (BASE/'fixture.stdout').write_bytes(b'READY\n' + stdout)
    (BASE/'fixture.stderr').write_bytes(stderr)
    record['fixture_exit_code'] = proc.returncode
    text = raw.decode(errors='replace')
    checks = {'debugger_exit_zero': debug.returncode == 0,
              'both_known_guest_frames': all(name in text for name in
                  ('diagnostic_condition_wait_marker', 'diagnostic_pipe_wait_marker')),
              'detach_reported': 'detached' in text.lower(),
              'fixture_normal_exit_after_detach': proc.returncode == 0}
    record['checks'] = checks
    record['status'] = 'PASS' if all(checks.values()) else 'FAIL'
except BaseException as error:
    record['status'] = 'FAIL'
    record['error_type'] = type(error).__name__
    record['error'] = str(error)
finally:
    if proc is not None and proc.poll() is None:
        if starttime(proc.pid) == record['actor']['starttime_ticks']:
            os.killpg(proc.pid, signal.SIGCONT)
            os.killpg(proc.pid, signal.SIGKILL)
        proc.wait(timeout=2)
    record['actor_closed'] = proc is None or proc.poll() is not None
    record['raw_sha256'] = {p.name: digest(p) for p in BASE.iterdir() if p.is_file() and p.name != 'preflight.json'}
    (BASE/'preflight.json').write_text(json.dumps(record, indent=2)+'\n')
    print(json.dumps(record, indent=2))
raise SystemExit(0 if record['status'] == 'PASS' else 1)
