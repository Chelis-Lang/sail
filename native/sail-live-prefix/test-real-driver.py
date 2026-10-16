"""Actual real-trace parent entry/children; solver sample driver is a fixture."""
import contextlib
import hashlib
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import subprocess
from unittest.mock import patch

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
WORKER = r'''import argparse,json,os,pathlib,hashlib
p=argparse.ArgumentParser();p.add_argument('--out',required=True);a,extra=p.parse_known_args();out=pathlib.Path(a.out);out.mkdir();case=os.environ['SAIL_TIME_TEST_CASE'];fresh='--fresh-only' in extra;experiment='--experiment-numeric-time-only' in extra
if fresh:
 data=b'unsat\n(:rlimit-count 10 :time 123.456)\n<<DONE>>\n'
 if case=='wrong-prefix':data=data.replace(b'10',b'11')
 for n in range(3):(out/('fresh-'+str(n)+'.stdout')).write_bytes(data)
 m={'fresh_control_pass':False,'experiment_fresh_control_pass':experiment and case not in ['fresh-error','missing-experiment-field']}
 if case=='missing-experiment-field':m.pop('experiment_fresh_control_pass')
else:
 m={'candidate_gate_pass':False,'experiment_candidate_gate_pass':experiment and case!='live-refused'}
 if case=='source-change':pathlib.Path('prefix-marker').write_text('changed')
 if case=='solver-change':pathlib.Path(os.environ['SAIL_TIME_TEST_SOLVER']).write_bytes(b'changed')
(out/'manifest.json').write_text(json.dumps(m))
raise SystemExit(7 if case=='live-exit' and not fresh or case=='fresh-nonzero' and fresh else 0)
'''
rows = []
for case in ['experiment-pass', 'strict-default', 'fresh-error', 'wrong-prefix',
             'live-refused', 'live-exit', 'missing-experiment-field',
             'source-change', 'solver-change', 'project-one', 'fresh-nonzero', 'fresh-timeout-exit0']:
    with tempfile.TemporaryDirectory(prefix='live real driver ') as td:
        root = Path(td)
        src = root / 'owned source'
        src.mkdir()
        (src / 'run.py').write_text(WORKER)
        (src / 'prefix-marker').write_text('original')
        z3 = root / 'owned-solver'
        z3.write_bytes(b'fixture image, never executed')
        solver_sha = hashlib.sha256(z3.read_bytes()).hexdigest()
        selected = []
        for project in ['ironkv', 'anvil']:
            data = src / project
            data.mkdir()
            (data / 'prefix.smt2').write_bytes(b'; original exact input prefix\n')
            (data / 'suffix.smt2').write_bytes(b'; original exact suffix\n')
            (data / 'prefix.stdout').write_bytes(b'unsat\n(:rlimit-count 10 :time 0.01)\n')
            selected.append({'project': project, 'edited.smt2_sha256': hashlib.sha256(
                (data / 'prefix.smt2').read_bytes() + (data / 'suffix.smt2').read_bytes()).hexdigest()})
        (src / 'pairs.json').write_text(json.dumps({'selected': selected}))
        hashes = {str(p.relative_to(src)): hashlib.sha256(p.read_bytes()).hexdigest()
                  for p in src.rglob('*') if p.is_file()}
        (src / 'source-hashes.json').write_text(json.dumps(hashes))
        spec = importlib.util.spec_from_file_location('real_driver', HERE / 'run-real.py')
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        module.SOLVER = solver_sha
        output = root / 'output'
        argv = [str(HERE / 'run-real.py'), '--source', str(src), '--out', str(output), '--z3', str(z3)]
        if case != 'strict-default':
            argv += ['--experiment-numeric-time-only']
        if case == 'project-one':
            argv += ['--project', 'anvil']
        raised = None
        with contextlib.ExitStack() as stack:
            stack.enter_context(patch.object(sys, 'argv', argv))
            stack.enter_context(patch.object(module.platform, 'system', lambda: 'Linux'))
            stack.enter_context(patch.object(module.platform, 'machine', lambda: 'x86_64'))
            stack.enter_context(patch.dict(module.os.environ, SAIL_TIME_TEST_CASE=case,
                                           SAIL_TIME_TEST_SOLVER=str(z3)))
            if case == 'fresh-timeout-exit0':
                original_wait = subprocess.Popen.wait
                observed = set()

                def wait(process, *args, **kwargs):
                    value = original_wait(process, *args, **kwargs)
                    if '--fresh-only' in process.args and process.pid not in observed:
                        observed.add(process.pid)
                        raise subprocess.TimeoutExpired(process.args, kwargs.get('timeout', 1100))
                    return value

                stack.enter_context(patch.object(subprocess.Popen, 'wait', wait))
            try:
                code = module.main()
            except BaseException as error:
                raised = type(error).__name__
                code = 1
        m = json.loads((output / 'manifest.json').read_text())
        wanted = case in ['experiment-pass', 'project-one']
        assert (code == 0) is wanted, (case, m)
        assert not m['qualified'] and not m['selected']
        assert bool(m['errors']) is not wanted
        if case in ['fresh-error', 'strict-default', 'wrong-prefix', 'missing-experiment-field',
                    'fresh-nonzero', 'fresh-timeout-exit0']:
            assert all('one_use_skipped' in row for row in m['projects'].values())
        if case == 'fresh-nonzero':
            assert all(row['fresh_exit'] == 7 and not row['fresh_process_pass'] for row in m['projects'].values())
        if case == 'fresh-timeout-exit0':
            assert all(row['fresh_exit'] == 0 and row['fresh_error'] == 'TimeoutExpired' and
                       not row['fresh_process_pass'] for row in m['projects'].values())
        if case in ['source-change', 'solver-change']:
            assert raised == 'AssertionError'
            assert not m['final_source_identity_stable'] if case == 'source-change' else not m['final_solver_identity_stable']
        if wanted:
            assert len(m['steps']) == (2 if case == 'project-one' else 4)
            if case == 'project-one':
                assert set(m['projects']) == {'anvil'}
            assert all(row['experiment_candidate_gate_pass'] for row in m['projects'].values())
            assert all(not row['fresh_control_pass'] and not row['candidate_gate_pass'] for row in m['projects'].values())
        rows.append({'case': case, 'passed': True, 'exit': code,
                     'scope': 'Actual parent and process entry; native solver/driver doubled explicitly.'})
print(json.dumps({'status': 'PASS', 'cases': len(rows), 'rows': rows}, indent=2))
