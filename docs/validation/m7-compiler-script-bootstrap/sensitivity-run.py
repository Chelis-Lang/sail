#!/usr/bin/env python3
"""Native root-owned bootstrap on/off/on; reuses the existing phase lifecycle."""
import argparse
import copy
import difflib
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import signal
import sys

sys.dont_write_bytecode = True
FILE = 'crates/sail-verus/src/managed/build_inputs_bootstrap.rs'
CASE = 'managed::build_inputs::family::bootstrap::tests::bootstrap_actual_cargo_can_publish_ordinary_contract_without_current_proof'
MESSAGE = 'independent ordinary-script contract must escape absent-generation bootstrap'
HELPER = 'f5dc0e498629e1945246c4ae7919d8b81f497783705a69139021c4575f08a5bc'
MAX_LOG = 32 * 1024 * 1024
BUILD = ['cargo', 'test', '--locked', '-p', 'sail-verus', '--lib', '--no-run', '--message-format=json']


def load_module(path, name):
    spec = importlib.util.spec_from_file_location(name, path)
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


def bounded(path):
    with path.open('rb') as handle:
        data = handle.read(MAX_LOG + 1)
    if len(data) > MAX_LOG:
        raise ValueError('capture exceeds 32 MiB')
    return data


def executable(log):
    paths = []
    for line in log.splitlines():
        if len(line) > 1024 * 1024:
            raise ValueError('compiler message exceeds 1 MiB')
        try:
            row = json.loads(line)
        except (ValueError, UnicodeDecodeError):
            continue  # Cargo progress is preserved raw; it grants no image identity.
        if (isinstance(row, dict) and row.get('reason') == 'compiler-artifact'
                and row.get('profile', {}).get('test') is True
                and row.get('target', {}).get('name') == 'sail_verus'):
            path = row.get('executable')
            if not isinstance(path, str) or not Path(path).is_absolute():
                raise ValueError('test artifact executable unavailable')
            paths.append(path)
    if len(paths) != 1:
        raise ValueError('one actual compiler-emitted libtest executable required')
    return paths[0]


def outcome(code, log, negative):
    text = log.decode('utf-8', errors='strict')
    label, passed, failed = ('FAILED', 0, 1) if negative else ('ok', 1, 0)
    summary = rf'test result: {re.escape(label)}\. {passed} passed; {failed} failed; 0 ignored; 0 measured;'
    if (code != (101 if negative else 0) or 'running 1 test' not in text
            or f'test {CASE} ... {label}' not in text or not re.search(summary, text)
            or negative and MESSAGE not in text):
        raise ValueError('unchanged exact-case sensitivity oracle failed')


def patched(original, patch):
    text = original.decode('utf-8')
    line = 'super::publish_records(export, compiler, records);'
    if text.count(line) != 1:
        raise ValueError('one exact independent emission required')
    negative = text.replace(line, 'let _ = (export, compiler, records);')
    expected = ''.join(difflib.unified_diff(text.splitlines(True), negative.splitlines(True),
                                          fromfile='a/' + FILE, tofile='b/' + FILE)).encode()
    if patch != expected:
        raise ValueError('immutable patch is not exactly remove-only emission')
    return negative.encode()


def replace_source(runner, check, source, state, original, content):
    runner.require_idle()  # Sticky uncertainty forbids mutation, including restoration.
    check()
    expected = copy.deepcopy(original)
    expected['files'][FILE].update(bytes=len(content), sha256=hashlib.sha256(content).hexdigest())
    (source / FILE).write_bytes(content)
    state['inventory'] = expected
    check()


def prerequisite(config, grant, sha):
    if (grant.get('status') != 'GRANTED'
            or grant.get('scope') != 'ordinary-script-bootstrap-sensitivity'
            or not re.fullmatch('[0-9a-f]{40}', config.get('revision', ''))):
        raise ValueError('exact root sensitivity grant required')
    for key in ('revision', 'source', 'target', 'pid1_birth', 'container', 'archive_sha256',
                'inventory_sha256', 'protected_sha256', 'patch_sha256', 'controller_sha256'):
        if config.get(key) is None or config[key] != grant.get(key):
            raise ValueError('grant binding differs: ' + key)
    if config['controller_sha256'] != sha(Path(__file__)):
        raise ValueError('controller bytes differ')
    if grant.get('holder_audit') != {'survivors': [], 'unavailable': []}:
        raise ValueError('complete root-owned preflight holder audit required')
    if not isinstance(config['container'], dict) or set(config['container']) != {'id', 'image', 'birth'}:
        raise ValueError('root transport container identity required')


def native(root):
    # Payload/preflight are staged by the root coordinator; this caller neither
    # inspects Docker nor grants target ownership. Native PID1 checks invalidate
    # the externally checked container binding after a restart.
    helper = root / 'local-combined-gates.py'
    if hashlib.sha256(helper.read_bytes()).hexdigest() != HELPER:
        raise ValueError('existing lifecycle helper bytes differ')
    g = load_module(helper, 'bootstrap_owned_phases')
    config, grant = g.load(root / 'config.json'), g.load(root / 'grant.json')
    prerequisite(config, grant, g.sha)
    if os.environ.get('CI') == 'true' or (root / 'primary.json').exists():
        raise ValueError('fresh local-only sensitivity scope required')
    for name, key in [('source.tar', 'archive_sha256'), ('source-inventory.json', 'inventory_sha256'),
                      ('protected.json', 'protected_sha256'), ('disable-publication.patch', 'patch_sha256')]:
        if g.sha(root / name) != config[key]:
            raise ValueError('frozen payload differs: ' + name)
    source, target = Path(config['source']), Path(config['target'])
    if (not source.is_absolute() or source.exists() or source.is_symlink()
            or not target.is_absolute() or not target.is_dir() or source == target
            or target in source.parents or source in target.parents):
        raise ValueError('new independent source and external existing target required')
    original = g.load(root / 'source-inventory.json')
    protected = g.load(root / 'protected.json')
    if not protected.get('files'):
        raise ValueError('protected engine/product/tested images required')
    source.mkdir()
    g.extract_source(root / 'source.tar', source, original)
    (source / 'target').symlink_to(target)
    module = g.support_module(root)
    owner = module.row(os.getpid())
    if owner is None:
        raise ValueError('controller birth unavailable')
    g.save(root / 'owner.json', owner)  # Existing cancel_native can stop this caller.
    state = {'inventory': original}
    record = {'status': 'RUNNING', 'revision': config['revision'], 'archive_sha256': config['archive_sha256'],
              'container': config['container'], 'pid1_birth': config['pid1_birth'], 'phases': [], 'images': [],
              'scope': 'fresh on/off/on component; no installed product, proof or performance credit',
              'environment_sha256': hashlib.sha256(json.dumps(dict(os.environ), sort_keys=True, ensure_ascii=True).encode()).hexdigest(),
              'cargo_incremental': '0'}
    closure = g.Closure()
    def check():
        g.verify_source(source, state['inventory'])
        g.verify_protected(protected)
        if os.readlink(source / 'target') != str(target) or g.sha(root / 'source.tar') != config['archive_sha256']:
            raise ValueError('source archive/target binding changed')
        first = module.row(1)
        if first is None or first['start'] != config['pid1_birth']:
            raise ValueError('container PID1 birth changed')
    runner = g.OwnedPhaseRunner(root, source, str(target), module, closure, record, check)
    for number in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
        signal.signal(number, runner.interrupt)
    os.environ['CARGO_INCREMENTAL'] = '0'
    try:
        check()
        positive = (source / FILE).read_bytes()
        negative = patched(positive, bounded(root / 'disable-publication.patch'))
        for phase, content, is_negative in [('positive', positive, False), ('negative', negative, True),
                                            ('restored', positive, False)]:
            if phase != 'positive':
                replace_source(runner, check, source, state, original, content)
            if runner.run(phase + '-compile', BUILD, 600) != 0:
                raise ValueError('actual compile failed: ' + phase)
            path = Path(executable(bounded(root / (phase + '-compile.log'))))
            if path.is_symlink() or not path.resolve().is_relative_to(target.resolve()):
                raise ValueError('compiler test image outside owned target')
            retained_total = sum(row['retention']['files'][0]['bytes'] for row in record['images'])
            if path.stat().st_size + retained_total > g.MAX_FILE:
                raise ValueError('all three tested images exceed 512 MiB retention cap')
            image_root = root / (phase + '-image')
            image_root.mkdir()
            retained = g.retained_images(image_root, [str(path)])
            if retained['errors'] or len(retained['files']) != 1:
                raise ValueError('actual executable retention failed')
            image = image_root / 'retained-images' / retained['files'][0]['copy']
            record['images'].append(dict(phase=phase, retention=retained, source_sha256=g.sha(source / FILE)))
            code = runner.run(phase + '-test', [str(image), CASE, '--exact', '--test-threads=1'], 180)
            outcome(code, bounded(root / (phase + '-test.log')), is_negative)
            if g.sha(image) != retained['files'][0]['sha256']:
                raise ValueError('executed retained image changed')
        runner.require_idle()
        check()
        if state['inventory'] != original:
            raise ValueError('restored source inventory differs')
        record['status'] = 'PASS'
    except BaseException as error:
        record.update(status='FAIL', error_type=type(error).__name__, error=str(error))
    finally:
        record['uncertainty'] = closure.reasons
        g.save(root / 'primary.json', record)  # Primary precedes final independent checks.
        try:
            runner.require_idle()
            check()
            record['closure'] = 'known actor groups closed; exact current source/protected bytes and PID1 unchanged'
            record['source_restored'] = state['inventory'] == original
        except BaseException as error:
            record.update(status='FAIL', closure='unavailable', closure_error=str(error))
        g.save(root / 'result.json', record)
    return 0 if record['status'] == 'PASS' else 1


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, required=True)
    args = parser.parse_args()
    raise SystemExit(native(args.root))
