#!/usr/bin/env python3
"""Owner-selected same-input byte oracle plus mandatory stock verdict/diagnostics.

Independent stock transcript differences are reported with both raw files. The
historical independent-stock qualification failure remains unchanged.

The project input contains tasks/lib.rs and the pinned tasks/*.rs sources. The
generated library wrapper must be the same one used by the corpus runner. Every
invocation uses one private source copy; the fetched corpus is never changed.
This source gate is independent of the CLI repair performance acceptance.
"""
import argparse
import hashlib
import json
import os
import pathlib
import platform
import shutil
from importlib.machinery import SourceFileLoader

oracle = SourceFileLoader('oracle', str(pathlib.Path(__file__).parent.parent / 'test-engine.py')).load_module()
controls = SourceFileLoader('same_callback_controls', str(pathlib.Path(__file__).with_name('test-same-callback.py'))).load_module()


def run(args):
    root = pathlib.Path(args.out).absolute()
    root.mkdir(parents=True, exist_ok=True)
    source = root / 'project'
    assert not source.exists(), 'Use a new output directory; retained cache/evidence must not be overwritten'
    shutil.copytree(pathlib.Path(args.project).absolute() / 'tasks', source / 'tasks')
    assert (source / 'tasks/lib.rs').is_file(), 'The corpus-generated library wrapper is required'
    results = []
    summary = {'version': 1, 'machine': platform.platform(), 'scope': 'Source callback oracle, not product performance',
               'patch_sha256': oracle.sha(pathlib.Path(__file__).with_name('graph-ledger-oracle.patch')),
               'engine_sha256': oracle.sha(pathlib.Path(args.engine)),
               'engine_verifier_sha256': oracle.sha(pathlib.Path(args.engine).with_name('rust_verify')),
               'stock_sha256': oracle.sha(pathlib.Path(args.stock)), 'z3_sha256': oracle.sha(pathlib.Path(args.z3)),
               'results': results}
    summary['same_input_byte_gate'] = 'actual ordered solver-writer bytes from independent current compiler owners'
    summary['qualified'] = False
    for revision in range(3 if args.other_module_edit else 2):
        if revision == 2:
            edited = source / 'tasks/human_eval_023.rs'
            original = edited.read_text()
            assert original.count('    string.len()\n') == 1, 'Frozen strlen refactor no longer matches'
            edited.write_text(original.replace('    string.len()\n', '    let length = string.len();\n    length\n'))
        work = root / str(revision)
        work.mkdir()
        records = {}
        for mode, actor in [('stock', args.stock), ('engine', args.engine)]:
            env = {key: value for key, value in os.environ.items() if not key.startswith('SAIL_ENGINE')}
            env.update(VERUS_Z3_PATH=args.z3, RUSTUP_TOOLCHAIN=args.toolchain)
            if mode == 'engine':
                env.update(SAIL_ENGINE='interactive', SAIL_ENGINE_FUNCTION='human_eval_verus::human_eval_013::compute_gcd',
                           SAIL_ENGINE_CONTEXT='real-he-oracle-v1:' + summary['engine_verifier_sha256'],
                           SAIL_ENGINE_PREPARED_DIR=str(root / 'prepared'), SAIL_ENGINE_EVENTS=str(work / 'events.jsonl'),
                           SAIL_ENGINE_FOCUS_REPORT=str(work / 'focus.json'),
                           SAIL_ENGINE_ORACLE_DIR=str(work / 'oracle'),
                           SAIL_ENGINE_ORACLE_Z3=args.z3)
            command = [actor, 'tasks/lib.rs', '--crate-type', 'lib', '--crate-name', 'human_eval_verus', '-V', 'use-crate-name',
                       '--output-json', '--time', '--no-report-long-running', '--log', 'air', '--log', 'smt-transcript',
                       '--log-dir', str(work / mode)]
            process = controls.execute(command, env, args.timeout, work, mode, cwd=source)
            (work / (mode + '.stdout')).write_bytes(process.stdout)
            (work / (mode + '.stderr')).write_bytes(process.stderr)
            records[mode] = process
        stock, engine = records['stock'], records['engine']
        a, b = json.loads(stock.stdout), json.loads(engine.stdout)
        left = sorted((work / 'stock').glob('*.smt_transcript'))
        right = sorted((work / 'engine').glob('*.smt_transcript'))
        events = [json.loads(line) for line in (work / 'events.jsonl').read_text().splitlines()]
        stats = next(event['payload'] for event in events if event['event'] == 'preparation-summary')
        callback_checks = [json.loads(path.read_text()) for path in sorted((work / 'oracle').glob('*/summary.json'))]
        actual_hits = sum(event['event'] == 'prepared-hit' for event in events)
        mismatches = [x.name for x, y in zip(left, right) if oracle.pipe(x) != oracle.pipe(y)]
        item = {'revision': revision, 'exit': engine.returncode, 'full_verification': b['verification-results'],
                'preparation': stats, 'SMT_sessions': len(right), 'SMT_mismatches': mismatches,
                'input_hashes': [{'name': p.name, 'sha256': hashlib.sha256(oracle.pipe(p)).hexdigest()} for p in right],
                'source_files': [{'name': str(p.relative_to(source)), 'sha256': oracle.sha(p)} for p in sorted(source.rglob('*.rs'))],
                'same_callback_checks': callback_checks, 'actual_hits': actual_hits,
                'stock_transcript_instability': [{'name': x.name, 'stock_bytes': len(oracle.pipe(x)),
                    'stock_sha256': hashlib.sha256(oracle.pipe(x)).hexdigest(),
                    'engine_bytes': len(oracle.pipe(y)), 'engine_sha256': hashlib.sha256(oracle.pipe(y)).hexdigest()}
                    for x, y in zip(left, right) if oracle.pipe(x) != oracle.pipe(y)]}
        results.append(item)
        (root / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
        assert stock.returncode == engine.returncode, (revision, 'exit')
        assert stock.stderr == engine.stderr, (revision, 'diagnostics')
        assert a['verification-results'] == b['verification-results'], (revision, 'summary')
        assert a['func-details'] == b['func-details'], (revision, 'functions')
        assert [p.name for p in left] == [p.name for p in right], (revision, 'session inventory')
        # Explicit owner decision: stock verdict/diagnostics remain mandatory;
        # the exact-byte gate compares cached/fresh lowering from SAME input.
        # Independent stock wire differences are retained above, not normalized.
        assert actual_hits > 0 and len(callback_checks) == actual_hits, (revision, 'unexamined restored hit')
        assert all(check['same_callback'] and check['restored_hit'] and check['passed']
                   for check in callback_checks), (revision, 'same-input raw/effect/diagnostic comparison')
        assert any(check['fresh_sessions'] > 0 for check in callback_checks), (revision, 'no actual captured solver session')
        assert stats['batch_hits'] + stats['persistent_hits'] > 0, (revision, 'no preparation restore')
        if revision == 1:
            assert any(event['event'] == 'prepared-restored' and event['phase'] == 'focus'
                       and event['payload']['source'] == 'persistent' for event in events), 'Focused persistent restore missing'
        assert events[-1]['event'] == 'compiler-sealed', 'Full compiler terminal seal missing'
        assert events[-1]['payload']['normal_termination'] is True
        item['assertions_completed'] = True
        summary['qualified'] = revision == (2 if args.other_module_edit else 1)
        (root / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
        print(revision, stats, flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    for flag in ['engine', 'stock', 'z3', 'project', 'out']:
        parser.add_argument('--' + flag, required=True)
    parser.add_argument('--toolchain', default='1.98.1')
    parser.add_argument('--timeout', type=int, default=600)
    parser.add_argument('--other-module-edit', action='store_true')
    run(parser.parse_args())
