#!/usr/bin/env python3
"""Bounded genuine previous-revision focus restore, with full module stock controls.

This additive source gate does not replace full HE/Vest or product/performance
acceptance. Each compiler invocation remains bounded by 600 seconds.
"""
import argparse
import hashlib
import json
import os
import pathlib
import shutil
from importlib.machinery import SourceFileLoader

ROOT = pathlib.Path(__file__).resolve().parent
oracle = SourceFileLoader('source_oracle', str(ROOT.parent / 'test-engine.py')).load_module()
controls = SourceFileLoader('same_callback', str(ROOT / 'test-same-callback.py')).load_module()


def run(args):
    root = pathlib.Path(args.out).absolute()
    root.mkdir(parents=True, exist_ok=False)
    project = root / 'project'
    shutil.copytree(pathlib.Path(args.project).absolute() / 'tasks', project / 'tasks')
    edit = project / 'tasks/human_eval_023.rs'
    original = edit.read_bytes()
    assert original.count(b'    string.len()\n') == 1
    altered = original.replace(b'    string.len()\n', b'    let length = string.len();\n    length\n')
    records = []
    summary = {'version': 1, 'scope': 'Module013 source proof and genuine prior-revision focused preparation; not whole-project or product qualification',
               'engine_verifier_sha256': oracle.sha(pathlib.Path(args.engine).with_name('rust_verify')),
               'patch_sha256': oracle.sha(pathlib.Path(args.patch)), 'results': records, 'controls_completed': False}
    def save():
        (root / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
    try:
        for revision, content in enumerate([original, original, altered]):
            edit.write_bytes(content)
            work = root / str(revision)
            work.mkdir()
            before = [{'name': str(p.relative_to(root / 'prepared')), 'sha256': oracle.sha(p)}
                      for p in sorted((root / 'prepared').rglob('*')) if p.is_file()]
            item = {'revision': revision, 'prior_preparations': before,
                    'edited_source_sha256': hashlib.sha256(content).hexdigest(), 'assertions_completed': False}
            records.append(item)
            save()
            actors = {}
            for arm, binary in [('stock', args.stock), ('engine', args.engine)]:
                env = {k: v for k, v in os.environ.items() if not k.startswith('SAIL_ENGINE')}
                env.update(VERUS_Z3_PATH=args.z3, RUSTUP_TOOLCHAIN='1.98.1')
                if arm == 'engine':
                    env.update(SAIL_ENGINE='interactive', SAIL_ENGINE_FUNCTION='human_eval_verus::human_eval_013::compute_gcd',
                               SAIL_ENGINE_CONTEXT='private-span-cross-edit-v1:' + summary['engine_verifier_sha256'],
                               SAIL_ENGINE_PREPARED_DIR=str(root / 'prepared'), SAIL_ENGINE_EVENTS=str(work / 'events.jsonl'),
                               SAIL_ENGINE_FOCUS_REPORT=str(work / 'focus.json'), SAIL_ENGINE_ORACLE_DIR=str(work / 'oracle'),
                               SAIL_ENGINE_ORACLE_Z3=args.z3)
                command = [binary, str(project / 'tasks/lib.rs'), '--crate-type', 'lib', '--crate-name', 'human_eval_verus',
                           '-V', 'use-crate-name', '--verify-only-module', 'human_eval_013', '--output-json', '--time',
                           '--no-report-long-running', '--log', 'air', '--log', 'smt-transcript', '--log-dir', str(work / arm)]
                process = controls.execute(command, env, 600, work, arm)
                (work / (arm + '.stdout')).write_bytes(process.stdout)
                (work / (arm + '.stderr')).write_bytes(process.stderr)
                actors[arm] = process
            stock, engine = actors['stock'], actors['engine']
            a, b = json.loads(stock.stdout), json.loads(engine.stdout)
            events = [json.loads(line) for line in (work / 'events.jsonl').read_text().splitlines()]
            checks = [json.loads(p.read_text()) for p in sorted((work / 'oracle').glob('*/summary.json'))]
            hits = sum(e['event'] == 'prepared-hit' for e in events)
            focused = [e for e in events if e['phase'] == 'focus' and e['event'] == 'prepared-restored']
            item.update(exit_code=engine.returncode, full_module_verification=b['verification-results'], actual_hits=hits,
                        focus_restores=focused, same_callback_checks=checks,
                        preparation=next(e['payload'] for e in events if e['event'] == 'preparation-summary'))
            save()
            assert stock.returncode == engine.returncode == 0, 'stock/module proof failed'
            assert stock.stderr == engine.stderr, 'exact stock diagnostics changed'
            assert a['verification-results'] == b['verification-results'], 'module verdict changed'
            assert a['func-details'] == b['func-details'], 'function verdict changed'
            assert hits == len(checks) and hits > 0, 'unchecked actual hit'
            assert all(c['passed'] and c['restored_hit'] and c['same_callback'] for c in checks), 'same-input raw/effect/diagnostic mismatch'
            assert events[-1]['event'] == 'compiler-sealed' and events[-1]['payload']['normal_termination'] is True
            if revision:
                assert before, 'no previous completed revision preparation'
                assert focused and focused[0]['payload']['source'] == 'persistent', 'previous-revision FOCUS restore missing'
                first_restore = next(i for i, e in enumerate(events) if e['event'] == 'prepared-restored')
                assert events[first_restore]['phase'] == 'focus', 'same-invocation FULL restore is not previous-revision reuse'
            item['assertions_completed'] = True
            save()
        summary['controls_completed'] = True
        save()
    finally:
        edit.write_bytes(original)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ['engine', 'stock', 'z3', 'project', 'patch', 'out']:
        parser.add_argument('--' + name, required=True)
    run(parser.parse_args())
