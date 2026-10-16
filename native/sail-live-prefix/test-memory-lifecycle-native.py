"""Bounded native Linux owned-task/mm-teardown controls, before trace evaluation.

Original errors and every attempt are retained. An unavailable genuine teardown
window fails the control; it is never replaced by injected positive evidence.
"""
import argparse
import errno
import hashlib
import json
import os
import select
from pathlib import Path
import subprocess
import sys
import time

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import memory_lifecycle as lifecycle
from live_prefix import capture_running_image, resident_bytes

WORKER = r'''import mmap,os,sys
maps=[mmap.mmap(-1,4096) for _ in range(8192)]
for value in maps:value[0]=1
print('ready',flush=True)
sys.stdin.buffer.read(1)
os._exit(int(sys.argv[1]))
'''


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    if (os.uname().sysname, os.uname().machine) != ('Linux', 'x86_64'):
        parser.error('native x86-64 Linux only')
    args.out.mkdir(exist_ok=False)
    kernel = dict(uname=list(os.uname()), PF_EXITING=lifecycle.PF_EXITING,
        reference='Linux v6.8 source rationale; actual kernel behavior must pass genuine controls')
    for name in ['/proc/version', '/proc/version_signature']:
        try:
            with open(name) as stream:
                value = stream.read(65537)
            if len(value) > 65536:
                raise ValueError('kernel identity over budget')
            kernel[name] = value
        except OSError as error:
            kernel[name] = dict(error=type(error).__name__ + ': ' + str(error))
    records = []
    control_deadline = time.monotonic() + 90
    checks = []
    executable = str(Path(sys.executable).resolve())
    expected = hashlib.sha256(Path(executable).read_bytes()).hexdigest()

    def save():
        (args.out / 'results.json').write_text(json.dumps(dict(
            platform=list(os.uname()), kernel_identity=kernel, python_image_sha256=expected,
            source_sha256={name: hashlib.sha256((HERE / name).read_bytes()).hexdigest()
                           for name in ['memory_lifecycle.py', 'live_prefix.py', Path(__file__).name]},
            records=records, checks=checks,
            all_pass=bool(checks) and all(row['passed'] for row in checks)), indent=2) + '\n')

    def one(case, progressing=False):
        if time.monotonic() >= control_deadline:
            row = dict(case=case, control_error='whole-control90second deadline exceeded')
            records.append(row)
            return row
        child = subprocess.Popen([executable, '-c', WORKER, '7' if case == 'nonzero' else '0'],
                                 stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        pidfd = image = held = None
        row = dict(case=case, pid=child.pid)
        records.append(row)
        try:
            ready = bytearray()
            ready_deadline = min(control_deadline, time.monotonic() + 10)
            while b'\n' not in ready:
                remaining = ready_deadline - time.monotonic()
                if remaining <= 0 or not select.select([child.stdout], [], [], max(0, remaining))[0]:
                    raise TimeoutError('owned child readiness deadline')
                block = os.read(child.stdout.fileno(), 32)
                if not block or len(ready) + len(block) > 32:
                    raise ValueError('invalid owned child readiness output')
                ready.extend(block)
            assert bytes(ready) == b'ready\n'
            pidfd, image, stamp = capture_running_image(child.pid, expected)
            identity = lifecycle.bind_task_identity(child.pid, pidfd)
            rss, pss = resident_bytes(child.pid)
            assert rss > 0 and pss > 0
            row.update(identity=identity, prior_rss=rss, prior_pss=pss, image_stamp=stamp)
            held = os.open(f'/proc/{child.pid}/smaps_rollup', os.O_RDONLY | os.O_CLOEXEC)
            assert os.read(held, 65536)
            os.lseek(held, 0, os.SEEK_SET)
            if case in {'live-ESRCH', 'foreign-starttime', 'foreign-pid', 'missing-pidfd'}:
                error = OSError(errno.ESRCH, 'explicit negative injection while actual owned child is alive')
                row['error_origin'] = 'injected negative; child remains blocked on input'
            else:
                child.stdin.write(b'x')
                child.stdin.flush()
                if progressing:
                    deadline = time.monotonic() + 2
                    while time.monotonic() < deadline:
                        try:
                            resident_bytes(child.pid)
                        except OSError as observed:
                            error = observed
                            break
                        except KeyError:
                            # A blank/incomplete RSS result is not the supported disappearance proof.
                            continue
                    else:
                        raise RuntimeError('no genuine RSS disappearance before bounded deadline')
                else:
                    child.wait(timeout=2)
                    try:
                        os.read(held, 65536)
                    except OSError as observed:
                        error = observed
                    else:
                        raise RuntimeError('held smaps descriptor did not produce a genuine terminal error')
                row['error_origin'] = 'actual kernel smaps_rollup read'
            if case == 'foreign-starttime':
                identity = dict(identity, starttime=identity['starttime'] + 1)
            if case == 'foreign-pid':
                identity = dict(identity, pid=identity['pid'] + 1)
            if case == 'EIO-terminal':
                error = OSError(errno.EIO, 'explicit infrastructure-negative injection')
                row['error_origin'] = 'injected negative, actual child terminal'
            row['observation'] = lifecycle.terminal_observation(
                child, None if case == 'missing-pidfd' else pidfd, identity, error,
                0 if case == 'no-prior-RSS' else 1, time.monotonic() + 1)
            if case.startswith('live-'):
                row['alive_after_refusal'] = child.poll() is None
            if row['observation']['terminal_race']:
                row['stdout_remainder'], row['stderr'] = [data.decode('utf-8') for data in child.communicate(timeout=2)]
                assert not row['stdout_remainder'] and not row['stderr'] and child.returncode == 0
                row.update(complete_io=True, actual_exit=child.returncode)
            return row
        except BaseException as error:
            row['control_error'] = type(error).__name__ + ': ' + str(error)
            return row
        finally:
            if child.poll() is None:
                child.kill()
            child.wait(timeout=2)
            for stream in [child.stdin, child.stdout, child.stderr]:
                stream.close()
            for fd in [held, image, pidfd]:
                if fd is not None:
                    os.close(fd)
            save()

    try:
        row = one('already-terminal')
        checks.append(dict(case='actual-mm-release-terminal', passed=(not row.get('control_error') and
            row.get('complete_io') is True and row.get('actual_exit') == 0 and
            row.get('observation', {}).get('terminal_race') is True)))
        found = False
        for index in range(8):
            row = one('exit-in-progress-' + str(index), progressing=True)
            observation = row.get('observation', {})
            if (not row.get('control_error') and row.get('complete_io') is True and
                    row.get('actual_exit') == 0 and observation.get('terminal_race') and
                    not observation.get('pidfd_terminal_before') and
                    observation.get('observed_task', {}).get('flags', 0) & lifecycle.PF_EXITING):
                found = True
                break
        checks.append(dict(case='actual-mm-release-before-waitable', passed=found))
        for case in ['live-ESRCH', 'foreign-starttime', 'foreign-pid', 'missing-pidfd',
                     'no-prior-RSS', 'nonzero', 'EIO-terminal']:
            row = one(case)
            checks.append(dict(case=case, passed=(not row.get('control_error') and
                row.get('observation', {}).get('terminal_race') is False and
                (case != 'live-ESRCH' or row.get('alive_after_refusal') is True))))
    finally:
        save()
    print(json.dumps(dict(checks=checks, attempts=len(records))))
    return 0 if all(row['passed'] for row in checks) else 1


if __name__ == '__main__':
    raise SystemExit(main())
