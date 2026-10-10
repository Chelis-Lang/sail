#!/usr/bin/env python3
"""Local-only source qualification for a separately ordinarily verified candidate.

Does not build or activate a runtime. Product/Vest performance and operational
acceptance remain separate mandatory gates, even when these source gates pass.
"""
import argparse
import hashlib
import json
import os
import pathlib
import platform
import subprocess
import time

ROOT = pathlib.Path(__file__).resolve().parent.parent


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(args):
    out = pathlib.Path(args.out).absolute()
    out.mkdir(parents=True, exist_ok=False)
    build = pathlib.Path(args.build).absolute()
    manifest_path = build / 'sail-build.json'
    manifest = json.loads(manifest_path.read_text())
    patch = ROOT / 'candidates/graph-ledger-oracle.patch'
    assert manifest['commit'] == '426d8b01e7ffb910a36c15e1f870ff985bb61eef'
    assert manifest['rust_toolchain'] == '1.98.1'
    assert manifest['features'][0] == 'sail-interactive:' + sha(patch)
    assert manifest['z3_sha256'] == sha(pathlib.Path(args.z3))
    for relative, digest in manifest['artifacts'].items():
        assert sha(build / relative) == digest, ('runtime artifact', relative)
    runtime = build / 'verus/source/target-verus/release'
    source = build / 'verus/source'
    record = {'version': 1, 'scope': 'Source gates only; no product qualification or activation',
              'patch_sha256': sha(patch), 'manifest_sha256': sha(manifest_path),
              'artifacts': manifest['artifacts'], 'ordinary_build_seconds': manifest['build_seconds'],
              'execution': args.execution, 'machine': platform.platform(),
              'stages': [], 'source_gates_passed': False, 'product_qualified': False}
    def save():
        (out / 'pipeline.json').write_text(json.dumps(record, indent=2) + '\n')
    def stage(name, command, cwd=ROOT):
        item = {'name': name, 'command': list(map(str, command)), 'status': 'running'}
        record['stages'].append(item)
        save()
        started = time.monotonic()
        with (out / (name + '.log')).open('wb') as log:
            result = subprocess.run(list(map(str, command)), cwd=cwd, env=environment,
                                    stdout=log, stderr=subprocess.STDOUT)
        item.update(status='passed' if result.returncode == 0 else 'failed',
                    exit_code=result.returncode, elapsed_seconds=time.monotonic() - started)
        save()
        print(name, item['status'], flush=True)
        if result.returncode:
            raise SystemExit(result.returncode)
    environment = {key: value for key, value in os.environ.items()
                   if not key.startswith('SAIL_ENGINE')}
    for key in ['RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'RUSTC', 'RUSTDOC',
                'RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER', 'CARGO_BUILD_TARGET_DIR']:
        environment.pop(key, None)
    environment.update(RUSTUP_TOOLCHAIN='1.98.1', VARGO_TARGET_DIR=str(runtime),
                       VERUS_Z3_PATH=args.z3, CARGO_TARGET_DIR=str(source / 'target'),
                       CARGO_BUILD_JOBS='2')
    save()
    stage('private-graph-audit', ['python3', ROOT / 'candidates/audit-graph-ledger.py', '--source', source])
    common = ['--engine', runtime / 'verus', '--stock', args.stock, '--z3', args.z3]
    for name, script in [('same-input-hit-controls', ROOT / 'candidates/test-same-callback.py'),
                         ('source-fixtures', ROOT / 'test-engine.py'),
                         ('graph-checkpoint', ROOT / 'candidates/test-graph-ledger.py'),
                         ('cancel-restart', ROOT / 'test-cancel.py')]:
        stage(name, ['python3', script, *common, '--out', out / name])
    stage('first-input-delta', ['python3', ROOT / 'candidates/test-key-delta.py',
                              '--engine', runtime / 'verus', '--z3', args.z3,
                              '--project', args.he, '--out', out / 'first-input-delta'])
    stage('real-he-same-input', ['python3', ROOT / 'candidates/test-real-he-same-input.py', *common,
                                '--project', args.he, '--out', out / 'real-he-same-input',
                                '--other-module-edit', '--timeout', '600'])
    for name, package, extra in [('vir-tests', 'vir', []), ('air-writer-tests', 'air', ['sail_wire_tests']),
                                 ('driver-tests', 'rust_verify', [])]:
        stage(name, ['nice', '-n', '19', 'cargo', 'test', '--release', '-p', package, '--lib', *extra], source)
    record['source_gates_passed'] = True
    save()
    print('Source gates passed; product acceptance remains pending', flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ['build', 'stock', 'z3', 'he', 'out', 'execution']:
        parser.add_argument('--' + name, required=True)
    run(parser.parse_args())
