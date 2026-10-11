#!/usr/bin/env python3
"""Freeze one stock tspec transcript diagnostic; never launch verification."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tomllib

CORPUS_SHA = 'b036cbf0b93d13ea08c09be42164e44d92d2c51d99f5a4840862f7f0a720d6e9'
PINS_SHA = '61d054416ef5c1f538b71c966445eeaab105cdc37bc6160406cabc1e04a3d790'
HARNESS_SHA = '1f62c6c276158cdf3dc083337115323eff02a97d25df1806fcf7c558709b78bf'
SHIM_SHA = 'a337030eec201a11f0b351b55bf2000e3e1f50e28f9a47580461e1af090bb80b'
STOCK = {
    'verus': '2fe2fca366846ec5aaaaaf80edf061f7d19564c097cb9840c7638719e8bc1854',
    'cargo-verus': '5b99d7fb9d57e3c86a5d2f6cf6270bbd4109d2950375c6a94ed389a3d6db3941',
    'rust_verify': 'b8c11e7b626cc85d8a46b90036f0adfec443b7cde1c69d8b182175b92c708af7',
    'z3': 'e583c4186a45e72411fa2cb2048401eed03f0f8e5f24694676a8f6271a50b765',
}
EXTRA = ['--log', 'smt-transcript', '--log-dir', '{out}/smt-transcripts']
ALLOW_SAIL = {'SAIL_VERUS_HOME', 'SAIL_TEST_Z3'}


def sha(path):
    digest = hashlib.sha256()
    with path.open('rb') as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b''):
            digest.update(chunk)
    return digest.hexdigest()


def diagnostic_corpus(text):
    original = tomllib.loads(text)
    projects = [p for p in original['project'] if p['name'] == 'verismo']
    assert len(projects) == 1 and 'extra_verus_args' not in projects[0]
    assert projects[0]['commit'] == '6bc6dd326d8b9ca30ffec57a544b13b5fadf3cb3'
    assert projects[0]['cargo_verus_command'] == 'focus'
    assert projects[0]['build'] == 'cargo-verus'
    assert projects[0]['crate_roots'] == ['source/bitflags_verus/src', 'source/verismo_tspec/src']
    name = text.index('\nname = "verismo"\n')
    start = text.rfind('[[project]]', 0, name)
    end = text.find('[[project]]', name)
    if end == -1:
        end = len(text)
    block = text[start:end]
    needle = 'cargo_verus_command = "focus"\n'
    assert block.count(needle) == 1
    block = block.replace(needle, needle + 'extra_verus_args = ' + json.dumps(EXTRA) + '\n')
    updated = text[:start] + block + text[end:]
    parsed = tomllib.loads(updated)
    expected = json.loads(json.dumps(original))
    selected = next(p for p in expected['project'] if p['name'] == 'verismo')
    selected['extra_verus_args'] = EXTRA
    assert parsed == expected  # No pin, project, option or expectation changes.
    return updated


def inventory(root):
    result = {}
    total = 0
    for directory, dirs, names in os.walk(root):
        dirs[:] = sorted(n for n in dirs if n != '.git')
        for name in sorted(names):
            if name == '.sail-fetched':
                continue
            path = Path(directory) / name
            assert not path.is_symlink() and path.is_file()
            size = path.stat().st_size
            total += size
            assert len(result) < 10000 and total <= 128 * 1024 * 1024
            result[str(path.relative_to(root))] = {'bytes': size, 'sha256': sha(path)}
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, required=True)
    parser.add_argument('--conformance', type=Path, required=True)
    parser.add_argument('--source-evidence', type=Path, required=True)
    args = parser.parse_args()
    assert os.getuid() == 0 and Path('/proc').is_dir()
    assert args.root.is_absolute() and args.conformance.is_absolute()
    assert not args.root.exists()
    evidence = args.source_evidence
    source_objects = {}
    for name, digest in [('sail-run.rs', '59d7ca4686025280daee10cfd8f15f9a982739d49ea0e89e440a7638ae2aaa93'),
                         ('conformance-main.rs', '122cb24e7f8799133a4008066ca3330e1494268279645c9055f3d96e99c32578')]:
        assert sha(evidence / name) == digest
    for path in sorted(evidence.glob('*.rs')):
        source_objects[path.name] = {'bytes': path.stat().st_size, 'sha256': sha(path)}
    unexpected = sorted(k for k in os.environ if k.startswith('SAIL_') and k not in ALLOW_SAIL)
    assert not unexpected, 'unapproved inherited Sail selectors: ' + ','.join(unexpected)
    assert os.environ.get('SAIL_VERUS_HOME') == '/opt/verus'
    assert os.environ.get('SAIL_TEST_Z3') == '/opt/verus/verus-x86-linux/z3'
    assert os.environ.get('VERUS_Z3_PATH') in (None, '')
    original = Path('/tmp/sail-m7-core-validation/corpus')
    assert sha(original / 'corpus.toml') == CORPUS_SHA
    assert sha(original / 'toolchain.toml') == PINS_SHA
    shim = Path('/opt/sail-m7-product-012707a/sail-z3')
    assert sha(args.conformance) == HARNESS_SHA and sha(shim) == SHIM_SHA
    assert args.conformance.stat().st_uid == 0 and shim.stat().st_uid == 0
    artifacts = {}
    for path in [args.conformance, shim, *[Path('/opt/verus/verus-x86-linux') / n
                 for n in ['verus', 'cargo-verus', 'rust_verify', 'z3', 'vstd.vir',
                           'libvstd.rlib', 'libverus_builtin.rlib',
                           'libverus_builtin_macros.so', 'libverus_state_machines_macros.so']]]:
        assert path.is_file() and not path.is_symlink()
        artifacts[str(path)] = {'bytes': path.stat().st_size, 'sha256': sha(path)}
        if path.name in STOCK:
            assert artifacts[str(path)]['sha256'] == STOCK[path.name]
    corpus = Path('/root/.cache/sail/corpus/verismo-6bc6dd326d8b')
    verus = Path('/root/.cache/sail/corpus/verus-426d8b01e7ff')
    trees = {}
    for name, path, expected in [('verismo', corpus, '6bc6dd326d8b9ca30ffec57a544b13b5fadf3cb3'),
                                 ('verus', verus, '426d8b01e7ffb910a36c15e1f870ff985bb61eef')]:
        head = subprocess.run(['git', '-C', str(path), 'rev-parse', 'HEAD'], check=True,
                              capture_output=True).stdout.decode().strip()
        assert head == expected
        trees[name] = {'root': str(path), 'commit': head, 'files': inventory(path)}
    modified = diagnostic_corpus((original / 'corpus.toml').read_text())
    args.root.mkdir(mode=0o700)
    cwd = args.root / 'cwd'
    (cwd / 'corpus').mkdir(parents=True, mode=0o700)
    (cwd / 'corpus/corpus.toml').write_text(modified)
    (cwd / 'corpus/toolchain.toml').write_bytes((original / 'toolchain.toml').read_bytes())
    env = dict(os.environ)
    env['SAIL_UNIT_TIMEOUT'] = '3600'
    private = args.root / 'private-environment.json'
    with open(private, 'x', opener=lambda path, flags: os.open(path, flags, 0o600)) as file:
        json.dump(env, file, sort_keys=True, indent=2)
        file.write('\n')
    argv = [str(args.conformance), 'run', '--tier', 'standard', '--project', 'verismo',
            '--filter', '/source/verismo_tspec/src', '--jobs', '1', '--shim', str(shim),
            '--out', str(args.root / 'harness')]
    record = {'version': 1, 'scope': 'One selected-unit logging diagnostic; no acceptance credit',
              'status': 'PREPARED_NOT_EXECUTED', 'actor_launched': False,
              'freeze_source_sha256': sha(Path(__file__)), 'artifacts': artifacts,
              'source_objects': source_objects,
              'source_trees': trees, 'cwd': str(cwd), 'argv': argv,
              'original_corpus_sha256': CORPUS_SHA,
              'diagnostic_corpus_sha256': sha(cwd / 'corpus/corpus.toml'),
              'toolchain_sha256': PINS_SHA, 'added_verus_args': EXTRA,
              'private_environment_sha256': sha(private),
              'original_unit_timeout_seconds': 3600, 'jobs': 1,
              'original_plain_runs': 1, 'original_conditional_confirm_runs': 2,
              'conditional_sail_arm_if_plain_completes': True, 'manual_retry': False,
              'original_runner_sha256': '59d7ca4686025280daee10cfd8f15f9a982739d49ea0e89e440a7638ae2aaa93',
              'original_and_0f_runner_bytes_equal': True,
              'telemetry_limits': {'observer_output_bytes': 24 * 1024 * 1024,
                                   'archive_content_bytes': 512 * 1024 * 1024,
                                   'archive_files': 4096, 'archive_seconds': 120},
              'stock_log_growth_limited_by_observer': False,
              'execution_controller_pending_review': True}
    (args.root / 'freeze.json').write_text(json.dumps(record, indent=2) + '\n')
    print(json.dumps({'status': record['status'], 'freeze_sha256': sha(args.root / 'freeze.json'),
                      'diagnostic_corpus_sha256': record['diagnostic_corpus_sha256'],
                      'actor_launched': False}))


if __name__ == '__main__':
    main()
