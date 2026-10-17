#!/usr/bin/env python3
"""Actual driver/core and owned child I/O; image/RSS guards are fixture doubles.

These controls never qualify Z3, native image admission, performance or a
timing exception. The independent native core controls retain that scope.
"""
import argparse
import contextlib
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
from unittest.mock import patch

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import live_prefix

CASES = ('success', 'equal-nonzero', 'equal-signal', 'different-status',
         'fresh-fallback', 'cleanup-failed', 'nonzero-and-cleanup',
         'original-and-cleanup')
WORKER = r'''#!PYTHON
import os,signal,sys,time
from pathlib import Path
data=sys.stdin.buffer.read()
case=data.split(b'owned-case:',1)[1].strip().decode()
os.write(1,b'actual owned child\x00\xff\n')
os.write(2,b'actual owned diagnostic\x00\n')
time.sleep(.03)
if case=='equal-signal':os.kill(os.getpid(),signal.SIGTERM)
if case=='different-status':
 p=Path('fixture-count');count=int(p.read_text()) if p.exists() else 0
 p.write_text(str(count+1));sys.exit(17 if count==0 else 0)
sys.exit(17 if case in ['equal-nonzero','nonzero-and-cleanup'] else 0)
'''


def worker_case(case, driver, output):
    """Run the unchanged native process/pipe/thread paths with explicit doubles."""
    spec = importlib.util.spec_from_file_location('live_driver', driver)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    output.mkdir()
    executable = output / 'owned-worker'
    executable.write_text(WORKER.replace('PYTHON', sys.executable))
    executable.chmod(0o700)
    prefix = output / 'prefix'
    expected = output / 'expected'
    suffix = output / 'suffix'
    prefix.write_bytes(b'; actual prefix submitted to an owned child\n')
    expected.write_bytes(b'')
    suffix.write_bytes(b'owned-case:' + case.encode() + b'\n')
    original_close = live_prefix.OneUsePool.close
    closes = []

    def close(pool):
        original_close(pool)
        closes.append(True)
        if len(closes) == 2 and case in ('cleanup-failed', 'nonzero-and-cleanup', 'original-and-cleanup'):
            raise OSError('injected final cleanup error AFTER actual owned child cleanup')

    def identity_error(_path):
        raise OSError('injected original identity observation failure')

    os.chdir(output)
    sys.argv = [str(driver), '--z3', str(executable), '--solver-sha256',
                hashlib.sha256(executable.read_bytes()).hexdigest(),
                '--prefix', str(prefix), '--prefix-stdout', str(expected),
                '--suffix', str(suffix), '--out', str(output / 'sample'),
                '--preparation-window', '2', '--ttl', '5', '--run-timeout', '2',
                '--oracle-timeout', '2', '--fallback-timeout', '2']
    with contextlib.ExitStack() as stack:
        # The real core, subprocesses, pipe readers/writers, wait statuses and
        # cleanup execute. Only native-platform/image and RSS admission are
        # doubled so these driver controls can run without Linux or Z3.
        stack.enter_context(patch.object(os, 'uname', lambda: os.uname_result(('Linux', 'fixture', '', '', 'x86_64'))))
        stack.enter_context(patch.object(os, 'pidfd_open', lambda *_: None, create=True))
        stack.enter_context(patch.object(live_prefix.signal, 'pidfd_send_signal', lambda *_: None, create=True))
        stack.enter_context(patch.object(live_prefix, 'capture_running_image', lambda *_: (None, None, None)))
        stack.enter_context(patch.object(module, 'capture_running_image', lambda *_: (None, None, None)))
        stack.enter_context(patch.object(live_prefix.Entry, '_image_ok', lambda _: True))
        stack.enter_context(patch.object(live_prefix, 'resident_bytes', lambda _: (1024, 1024)))
        stack.enter_context(patch.object(module, 'resident_bytes', lambda _: (1024, 1024)))
        stack.enter_context(patch.object(live_prefix.OneUsePool, 'close', close))
        if case == 'fresh-fallback':
            stack.enter_context(patch.object(live_prefix.OneUsePool, 'seed', lambda *_: None))
        if case == 'original-and-cleanup':
            stack.enter_context(patch.object(module, 'path_digest', identity_error))
        return module.main()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--driver', type=Path, default=HERE / 'run.py')
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--worker-case', choices=CASES)
    args = parser.parse_args()
    driver, output = args.driver.resolve(strict=True), args.out.resolve()
    if args.worker_case:
        return worker_case(args.worker_case, driver, output)
    output.mkdir(parents=True, exist_ok=False)
    records = []
    for case in CASES:
        home = output / case
        completed = subprocess.run([sys.executable, '-B', str(Path(__file__).resolve()),
                                    '--driver', str(driver), '--out', str(home), '--worker-case', case],
                                   capture_output=True, timeout=20)
        (output / (case + '.stdout')).write_bytes(completed.stdout)
        (output / (case + '.stderr')).write_bytes(completed.stderr)
        record = {'case': case, 'driver_exit': completed.returncode, 'passed': False}
        try:
            sample = home / 'sample'
            manifest = json.loads((sample / 'manifest.json').read_text())
            wanted = 0 if case == 'success' else 1
            assert completed.returncode == wanted
            assert manifest['candidate_gate_pass'] == (case == 'success')
            if case != 'original-and-cleanup':
                assert manifest['candidate_exit'] == (-15 if case == 'equal-signal' else
                       17 if case in ('equal-nonzero', 'different-status', 'nonzero-and-cleanup') else 0)
                assert manifest['fresh_exit'] == (-15 if case == 'equal-signal' else
                       17 if case in ('equal-nonzero', 'nonzero-and-cleanup') else 0)
            for stream, value in [('stdout', b'actual owned child\x00\xff\n'),
                                  ('stderr', b'actual owned diagnostic\x00\n')]:
                assert (sample / ('candidate.' + stream)).read_bytes() == value
                assert (sample / ('fresh.' + stream)).read_bytes() == value
            if case == 'fresh-fallback':
                assert manifest['mode'] == 'fresh-fallback' and manifest['raw_equal']
            if case in ('cleanup-failed', 'nonzero-and-cleanup', 'original-and-cleanup'):
                assert any('injected final cleanup error' in error for error in manifest['cleanup_errors'])
            if case == 'original-and-cleanup':
                assert 'injected original identity observation failure' in manifest['error']
            record.update(passed=True, manifest=manifest)
        except (AssertionError, OSError, KeyError, ValueError) as error:
            record['error'] = repr(error)
        records.append(record)
        print(case, 'PASS' if record['passed'] else 'FAIL', flush=True)
    result = {'all_pass': all(row['passed'] for row in records), 'rows': records,
              'driver_sha256': hashlib.sha256(driver.read_bytes()).hexdigest(),
              'core_sha256': hashlib.sha256((HERE / 'live_prefix.py').read_bytes()).hexdigest(),
              'scope': __doc__}
    (output / 'controls.json').write_text(json.dumps(result, indent=2) + '\n')
    return 0 if result['all_pass'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
