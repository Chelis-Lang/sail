"""Actual opt-in driver/core child-I/O controls; native image/RSS are doubles."""
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

CASES = ['numeric-time', 'other-statistic', 'verdict', 'model', 'stderr',
         'equal-nonzero', 'memory-ESRCH', 'cleanup-failed', 'default-exact']
WORKER = r'''#!PYTHON
import os,sys,time
from pathlib import Path
case=sys.argv[1] if len(sys.argv)>1 and sys.argv[1]!='-smt2' else Path('case').read_text()
p=Path('count');n=int(p.read_text())+1 if p.exists() else 1;p.write_text(str(n))
sys.stdin.buffer.readline()
verdict=b'sat' if case=='verdict' and n==1 else b'unsat'
other=str(n if case=='other-statistic' else 10).encode()
os.write(1,verdict+b'\n(:rlimit-count '+other+b' :time '+(b'1.2' if n==1 else b'123.456')+b')\n')
sys.stdin.buffer.read()
os.write(1,b'(model (define-fun x () Int '+str(n if case=='model' else 5).encode()+b'))\n')
os.write(2,b'diagnostic '+str(n if case=='stderr' else 0).encode()+b'\n')
time.sleep(.1)
sys.exit(17 if case=='equal-nonzero' else 0)
'''


def worker(case, output):
    spec = importlib.util.spec_from_file_location('time_driver', HERE / 'run.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    output.mkdir()
    (output / 'case').write_text(case)
    exe = output / 'worker'
    exe.write_text(WORKER.replace('PYTHON', sys.executable))
    exe.chmod(0o700)
    prefix, suffix, expected = [output / name for name in ['prefix', 'suffix', 'expected']]
    prefix.write_bytes(b'; owned prefix\n')
    suffix.write_bytes(b'; exact suffix\n')
    expected.write_bytes(b'unsat\n(:rlimit-count 10 :time 0.01)\n')
    original_consume = live_prefix.OneUsePool.consume
    original_close = live_prefix.OneUsePool.close
    closes = []

    def consume(pool, key, data):
        result = original_consume(pool, key, data)
        if result is not None:
            assert result['prefix_stdout'] + result['suffix_stdout'] == result['stdout']
            (output / 'partition.json').write_text(json.dumps({
                'prefix': result['prefix_stdout'].decode(),
                'suffix': result['suffix_stdout'].decode()}))
        return result

    def close(pool):
        original_close(pool)
        closes.append(True)
        if len(closes) == 2 and case == 'cleanup-failed':
            raise OSError('injected final cleanup failure after actual cleanup')

    def rss(_pid):
        if case == 'memory-ESRCH':
            raise ProcessLookupError(3, 'actual fixture observation ESRCH')
        return 1024, 1024

    os.chdir(output)
    sys.argv = [str(HERE / 'run.py'), '--z3', str(exe), '--solver-sha256',
                hashlib.sha256(exe.read_bytes()).hexdigest(), '--prefix', str(prefix),
                '--prefix-stdout', str(expected), '--suffix', str(suffix),
                '--out', str(output / 'sample'), '--preparation-window', '2',
                '--ttl', '5', '--run-timeout', '2', '--oracle-timeout', '2',
                '--fallback-timeout', '2']
    if case != 'default-exact':
        sys.argv += ['--experiment-numeric-time-only']
    with contextlib.ExitStack() as stack:
        stack.enter_context(patch.object(os, 'uname', lambda: os.uname_result(('Linux', 'fixture', '', '', 'x86_64'))))
        stack.enter_context(patch.object(os, 'pidfd_open', lambda *_: None, create=True))
        stack.enter_context(patch.object(live_prefix.signal, 'pidfd_send_signal', lambda *_: None, create=True))
        stack.enter_context(patch.object(live_prefix, 'capture_running_image', lambda *_: (None, None, None)))
        stack.enter_context(patch.object(module, 'capture_running_image', lambda *_: (None, None, None)))
        stack.enter_context(patch.object(live_prefix.Entry, '_image_ok', lambda _: True))
        stack.enter_context(patch.object(live_prefix, 'resident_bytes', rss))
        stack.enter_context(patch.object(module, 'resident_bytes', rss))
        stack.enter_context(patch.object(live_prefix.OneUsePool, 'close', close))
        stack.enter_context(patch.object(live_prefix.OneUsePool, 'consume', consume))
        # Policy separation is explicit; default binding bytes keep their old shape.
        ordinary = live_prefix.OneUsePool(str(exe), hashlib.sha256(exe.read_bytes()).hexdigest())
        experiment = live_prefix.OneUsePool(str(exe), hashlib.sha256(exe.read_bytes()).hexdigest(),
                                            experiment_numeric_time_only=True)
        assert 'response_experiment' not in ordinary.binding
        assert experiment.binding == dict(ordinary.binding, response_experiment='numeric-time-only-experiment-v1')
        original_close(ordinary)
        original_close(experiment)
        return module.main()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--case', choices=CASES)
    args = parser.parse_args()
    output = args.out.resolve()
    if args.case:
        return worker(args.case, output)
    output.mkdir(exist_ok=False)
    rows = []
    for case in CASES:
        result = subprocess.run([sys.executable, '-B', str(Path(__file__).resolve()),
                                 '--case', case, '--out', str(output / case)],
                                capture_output=True, timeout=20)
        (output / (case + '.stdout')).write_bytes(result.stdout)
        (output / (case + '.stderr')).write_bytes(result.stderr)
        manifest = json.loads((output / case / 'sample/manifest.json').read_text())
        passed = case == 'numeric-time'
        assert result.returncode == (0 if passed else 1), (case, result.returncode, manifest)
        assert manifest.get('experiment_candidate_gate_pass', False) is passed, (case, manifest)
        assert manifest['candidate_gate_pass'] is False
        if passed:
            assert manifest['mode'] == 'one-use-live-prefix'
            assert manifest['raw_equal'] is False
            partition = json.loads((output / case / 'partition.json').read_text())
            assert partition['prefix'] == 'unsat\n(:rlimit-count 10 :time 1.2)\n'
            assert partition['suffix'] == '(model (define-fun x () Int 5))\n'
            c = manifest['experiment_comparison']
            assert c['left_time_tokens'][0]['value'] == '1.2'
            assert c['right_time_tokens'][0]['value'] == '123.456'
            assert (output / case / 'sample/candidate.stdout').read_bytes().endswith(b'(model (define-fun x () Int 5))\n')
        if case == 'memory-ESRCH':
            assert 'No such process' in json.dumps(manifest) or 'ESRCH' in json.dumps(manifest)
        if case == 'cleanup-failed':
            assert manifest['cleanup_errors']
        if case == 'equal-nonzero':
            assert manifest['candidate_exit'] == manifest['fresh_exit'] == 17
        rows.append({'case': case, 'passed': True, 'driver_exit': result.returncode,
                     'mode': manifest.get('mode'), 'raw_equal': manifest.get('raw_equal'),
                     'experiment_gate_pass': manifest.get('experiment_candidate_gate_pass', False)})
        print('PASS', case)
    (output / 'results.json').write_text(json.dumps({'status': 'PASS', 'cases': len(rows),
        'rows': rows, 'scope': 'Actual child I/O/driver/core; native executable/RSS observation doubled. Not native or performance qualification.'}, indent=2) + '\n')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
