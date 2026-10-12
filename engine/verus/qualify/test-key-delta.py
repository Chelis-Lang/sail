#!/usr/bin/env python3
"""Local-only exact preparation-component diagnostic on two focused HE callbacks.

It does not establish conformance or performance. It preserves both original
and edited input bytes and the complete ordered baseline component captures.
"""
import argparse
import hashlib
import json
import os
import pathlib
import shutil
from importlib.machinery import SourceFileLoader

controls = SourceFileLoader('same_callback_controls', str(pathlib.Path(__file__).with_name('test-same-callback.py'))).load_module()


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(args):
    root = pathlib.Path(args.out).absolute()
    root.mkdir(parents=True, exist_ok=False)
    project = root / 'project'
    shutil.copytree(pathlib.Path(args.project).absolute() / 'tasks', project / 'tasks')
    source = project / 'tasks/human_eval_023.rs'
    original = source.read_bytes()
    assert original.count(b'    string.len()\n') == 1
    edited = original.replace(b'    string.len()\n', b'    let length = string.len();\n    length\n')
    (root / 'original-human_eval_023.rs').write_bytes(original)
    (root / 'edited-human_eval_023.rs').write_bytes(edited)
    records = []
    try:
        for revision, data in enumerate([original, edited]):
            source.write_bytes(data)
            work = root / str(revision)
            work.mkdir()
            environment = {key: value for key, value in os.environ.items()
                           if not key.startswith('SAIL_ENGINE')}
            environment.update(VERUS_Z3_PATH=args.z3, RUSTUP_TOOLCHAIN=args.toolchain,
                               SAIL_ENGINE='interactive',
                               SAIL_ENGINE_FUNCTION='human_eval_verus::human_eval_013::compute_gcd',
                               SAIL_ENGINE_CONTEXT='key-delta-v1:' + sha(pathlib.Path(args.engine).with_name('rust_verify')),
                               SAIL_ENGINE_PREPARED_DIR=str(root / 'prepared'),
                               SAIL_ENGINE_EVENTS=str(work / 'events.jsonl'),
                               SAIL_ENGINE_FOCUS_REPORT=str(work / 'focus.json'),
                               SAIL_ENGINE_ORACLE_COMPONENTS=str(work / 'components'),
                               SAIL_ENGINE_ORACLE_COMPONENT_MODULE='human_eval_013')
            command = [args.engine, str(project / 'tasks/lib.rs'), '--crate-type', 'lib',
                       '--crate-name', 'human_eval_verus', '-V', 'use-crate-name',
                       '--verify-only-module', 'human_eval_013', '--output-json', '--time',
                       '--no-report-long-running']
            process = controls.execute(command, environment, args.timeout, work, 'engine')
            (work / 'stdout').write_bytes(process.stdout)
            (work / 'stderr').write_bytes(process.stderr)
            assert process.returncode == 0, 'Focused diagnostic proof failed'
            captures = sorted((work / 'components').glob('*/index.json'))
            assert captures, 'Exact baseline capture unavailable'
            records.append({'revision': revision, 'exit_code': process.returncode,
                            'changed_source_sha256': sha(source),
                            'full': json.loads(process.stdout)['verification-results'],
                            'captures': [json.loads(path.read_text()) for path in captures]})
        left, right = records[0]['captures'][0], records[1]['captures'][0]
        assert [entry['name'] for entry in left] == [entry['name'] for entry in right]
        differences = [a['name'] for a, b in zip(left, right)
                       if (a['bytes'], a['sha256']) != (b['bytes'], b['sha256'])]
        # Metadata identifies candidate differences. Confirm every component by
        # direct bytes before publishing the first changed typed component.
        first_dirs = [sorted((root / str(revision) / 'components').glob('*/index.json'))[0].parent
                      for revision in [0, 1]]
        direct = [a['name'] for a, b in zip(left, right)
                  if (first_dirs[0] / a['file']).read_bytes() != (first_dirs[1] / b['file']).read_bytes()]
        assert direct == differences
        result = {'version': 1, 'scope': 'Focused input diagnostic; not conformance or product performance',
                  'engine_verifier_sha256': sha(pathlib.Path(args.engine).with_name('rust_verify')),
                  'first_changed_component': direct[0] if direct else None,
                  'changed_components': direct, 'records': records}
        (root / 'summary.json').write_text(json.dumps(result, indent=2) + '\n')
        print('first changed component:', result['first_changed_component'], flush=True)
    finally:
        source.write_bytes(original)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ['engine', 'z3', 'project', 'out']:
        parser.add_argument('--' + name, required=True)
    parser.add_argument('--toolchain', default='1.98.1')
    parser.add_argument('--timeout', type=int, default=600)
    run(parser.parse_args())
