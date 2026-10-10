#!/usr/bin/env python3
"""Local-only persistent checkpoint oracle against an already built source engine."""
import argparse
import hashlib
import json
import os
import pathlib
import subprocess

from importlib.machinery import SourceFileLoader
oracle = SourceFileLoader('oracle', str(pathlib.Path(__file__).with_name('test-engine.py'))).load_module()


def run(args):
    root = pathlib.Path(args.out).absolute()
    root.mkdir(parents=True, exist_ok=True)
    source = root / 'lib.rs'
    source.write_text('use verus_builtin::*; use verus_builtin_macros::*; mod stable; mod edited; fn main() {}\n')
    stable = root / 'stable.rs'
    stable.write_text('use super::*; verus! { pub proof fn selected(x: int) ensures x == x { } }\n')
    edited = root / 'edited.rs'
    cache = root / 'prepared'
    cache.mkdir(exist_ok=True)
    results = []
    stable_body = 'use super::*; macro_rules! identity {($x:expr)=>{$x}} verus! { pub proof fn selected(x: int) ensures x > x { assert(identity!(false)); } }\n'
    edits = [('', ''), ('', ''), (' assert(true); ', ''), (' assert(true); ', '\n\n'), (' assert(true); ', '\n\n'), (' assert(true); ', '\n\n'), (' assert(true); ', '\n\n'), (' assert(true); ', '\n\n')]
    for index, (body, prefix) in enumerate(edits):
        if index == 5:
            for path in cache.glob('*.prepared'):
                data = bytearray(path.read_bytes())
                assert data, 'empty checkpoint'
                data[-1] ^= 1
                path.write_bytes(data)
        if index == 6:
            files = sorted(cache.glob('*.prepared'))
            bodies = [path.read_bytes() for path in files]
            assert len(set(bodies)) > 1, 'misfile oracle needs different checkpoint keys'
            for offset, path in enumerate(files):
                replacement = next(body for body in bodies[offset+1:] + bodies[:offset+1] if body != bodies[offset])
                path.write_bytes(replacement)
        if index == 7:
            for path in cache.glob('*.prepared'):
                with path.open('r+b') as output:
                    output.truncate(128*1024*1024 + 65)
        edited.write_text('use super::*; verus! { pub proof fn other(x: int) ensures x == x {' + body + '} }\n')
        stable.write_text(prefix + stable_body)
        outputs = {}
        work = root / str(index)
        work.mkdir(exist_ok=True)
        (work / 'events.jsonl').unlink(missing_ok=True)
        (work / 'focus.json').unlink(missing_ok=True)
        for mode, executable in [('stock', args.stock), ('engine', args.engine)]:
            env = {key: value for key, value in os.environ.items() if not key.startswith('SAIL_ENGINE')}
            env['VERUS_Z3_PATH'] = args.z3
            if mode == 'engine':
                env.update(SAIL_ENGINE='interactive', SAIL_ENGINE_FUNCTION='demo::stable::selected',
                           SAIL_ENGINE_CONTEXT='local-source-oracle-v1:' + oracle.sha(pathlib.Path(args.engine).with_name('rust_verify')),
                           SAIL_ENGINE_PREPARED_DIR=str(cache), SAIL_ENGINE_EVENTS=str(work / 'events.jsonl'),
                           SAIL_ENGINE_FOCUS_REPORT=str(work / 'focus.json'))
            command = [executable, str(source), '--no-vstd', '--crate-name', 'demo', '-V', 'use-crate-name',
                       '--output-json', '--time', '--log', 'air', '--log', 'smt-transcript', '--log-dir', str(work / mode),
                       '--no-report-long-running']
            result = subprocess.run(command, capture_output=True, env=env, timeout=120)
            (work / (mode + '.stdout')).write_bytes(result.stdout)
            (work / (mode + '.stderr')).write_bytes(result.stderr)
            outputs[mode] = result
        left, right = outputs['stock'], outputs['engine']
        assert left.returncode == right.returncode and left.stderr == right.stderr, (index, 'verdict/diagnostics')
        a, b = json.loads(left.stdout), json.loads(right.stdout)
        assert a['verification-results'] == b['verification-results'], (index, 'verification summary')
        assert a['func-details'] == b['func-details'], (index, 'function details')
        full = sorted((work / 'stock').glob('*.smt_transcript'))
        actual = sorted((work / 'engine').glob('*.smt_transcript'))
        assert [path.name for path in full] == [path.name for path in actual], (index, 'sessions')
        assert [oracle.pipe(path) for path in full] == [oracle.pipe(path) for path in actual], (index, 'SMT bytes')
        events = [json.loads(line) for line in (work / 'events.jsonl').read_text().splitlines()]
        focus_hits = [event for event in events if event['event'] == 'prepared-hit' and event['phase'] == 'focus']
        if index in [3,5,6,7]:
            assert not focus_hits, (index, 'stale, corrupt, misfiled or oversized checkpoint restored')
        hits = [event['payload']['bucket'] for event in events if event['event'] == 'prepared-hit']
        results.append({'revision':index, 'hits':hits, 'files':len(list(cache.glob('*.prepared'))),
                        'full_inputs':[{'name':path.name,'sha256':hashlib.sha256(oracle.pipe(path)).hexdigest()} for path in actual]})
        print(index, hits, flush=True)
    assert results[1]['hits'], 'unchanged callback did not restore any prepared bucket'
    assert any('stable' in bucket for bucket in results[2]['hits']), 'other-module edit failed to reuse stable lowering'
    (root / 'summary.json').write_text(json.dumps({'version':1,'engine_sha256':oracle.sha(pathlib.Path(args.engine)), 'engine_verifier_sha256':oracle.sha(pathlib.Path(args.engine).with_name('rust_verify')), 'results':results}, indent=2) + '\n')

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ['engine','stock','z3','out']:
        parser.add_argument('--' + name, required=True)
    run(parser.parse_args())
