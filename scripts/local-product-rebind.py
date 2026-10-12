#!/usr/bin/env python3
"""Revalidate existing installed bytes after restart; rebind only execution pointer.

No build, installation, proof, target mutation, receipt or cache reuse authority.
The producer and historical installation remain unchanged.
"""
import argparse
import copy
import hashlib
import importlib.util
import inspect
import json
import os
from pathlib import Path
import re
import signal
import stat
import sys
import time
import types

sys.dont_write_bytecode = True
CONTAINERS = ('sail-m7-acceptance', 'sail-m7-handoff-validation')
SUPPORT = {
    'scripts/inventory-old-benchmark-single-deps.py': 'c87cea181173c62af54ca3204669fc21cc8653e226d8bac285522df83b21081e',
    'scripts/local-combined-gates.py': 'f5dc0e498629e1945246c4ae7919d8b81f497783705a69139021c4575f08a5bc',
    'docs/validation/m7-current-benchmark-controller.py': 'b79112b5674b00dc3d16df8ca577c5873b471d48b2e41e3173f993a2ec09df90',
    'docs/validation/m7-current-benchmark-install.py': 'ef90e9218050f0a77f911fd951fd359ecd095a4668f805217d07f0d89dcf126f',
}
MAX_METADATA = 8 * 1024 * 1024


def need(ok, message):
    if not ok:
        raise ValueError(message)


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def unique(items):
    result = {}
    for key, value in items:
        need(key not in result, 'duplicate metadata key')
        result[key] = value
    return result


def read(path, limit=MAX_METADATA):
    path = Path(path)
    need(stat.S_ISREG(path.lstat().st_mode), 'metadata must be regular, not a link')
    with path.open('rb') as stream:
        raw = stream.read(limit + 1)
    need(len(raw) <= limit, 'metadata byte bound exceeded')
    return raw


def module(code, name):
    value = types.ModuleType(name)
    value.__file__ = name
    exec(compile(code, name, 'exec'), value.__dict__)
    return value


def support(repo):
    result = {}
    for name, expected in SUPPORT.items():
        raw = read(Path(repo) / name, 1024 * 1024)
        need(digest(raw) == expected, 'reviewed support bytes differ: ' + name)
        result[name] = raw.decode()
    return result


def closed(value):
    need(value.get('complete') is True and value.get('holders') == []
         and value.get('unavailable') == [], 'scoped holder closure unavailable')
    need(len(value.get('self_excluded', [])) == 1, 'exact census birth exclusion absent')


def same(before, after):
    need(before == after, 'container/birth/mount/PID1 or sealed byte binding changed')


def rebound(old, producer_sha, birth, evidence, historical_sha):
    """Keep historical phases verbatim; only new execution binding has authority."""
    need(old['producer_grant_sha256'] == producer_sha, 'producer changed')
    result = copy.deepcopy(old)
    result['execution_container_birth'] = birth
    result['scope'] = ('Existing byte-identical installation revalidated after restart; '
                       'no new build/install/proof or cache/target reuse authority')
    result['execution_revalidation'] = {
        'version': 1, 'authority': 'installation-only',
        'historical_execution_grant_sha256': historical_sha,
        'producer_grant_sha256': producer_sha,
        'evidence_sha256': evidence, 'build_performed': False,
        'installation_performed': False, 'proof_performed': False,
        'historical_pass_repeated': False,
    }
    return result


def replace_pointer(path, expected, raw):
    """Compare the designated old pointer again immediately before atomic update."""
    need(digest(read(path)) == expected, 'execution pointer changed before commit')
    temporary = path.with_name(path.name + '.rebind-new')
    descriptor = os.open(temporary, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    try:
        with os.fdopen(descriptor, 'wb') as handle:
            handle.write(raw)
            handle.flush()
            os.fsync(handle.fileno())
        need(digest(read(path)) == expected, 'execution pointer changed during commit')
        os.replace(temporary, path)
    finally:
        # Only our exclusively created temporary; the historical pointer is untouched on refusal.
        if temporary.exists():
            temporary.unlink()


def native(config):
    """Read-only evaluation using the exact audited holder and all-task guards."""
    code = config['support']
    for name in SUPPORT:
        need(digest(code[name].encode()) == SUPPORT[name], 'native support differs')
    holders = module(code['scripts/inventory-old-benchmark-single-deps.py'], 'holder_guard')
    gates = module(code['scripts/local-combined-gates.py'], 'task_guard')
    benchmark = module(code['docs/validation/m7-current-benchmark-controller.py'], 'benchmark_guard')
    grant = benchmark.validate_grant(config['grant'], config['revision'])
    source = Path(grant['source_path'])
    who = os.geteuid()
    need(not source.is_symlink() and source.is_dir(), 'source root changed type')
    need(source.stat().st_uid == who and stat.S_IMODE(source.stat().st_mode) == 0o700,
         'source root ownership/private mode differs')
    # Identical original whole-tree verifier, without import writes into the source.
    verifier = module('from pathlib import Path\nimport os,stat\n' + config['verify_source'], 'source_guard')
    verifier.require, verifier.sha = need, benchmark.sha
    inventory = dict(config['inventory'])
    if config['producer']:
        # This original gate link is deliberately outside the Git archive. No
        # arbitrary extra tracked entry or target directory is admitted.
        need('target' not in inventory, 'unexpected archived target')
        inventory['target'] = {'kind': 'symlink', 'target': '/work/target', 'mode': 0o777}
    verifier.verify_source(source, inventory)
    auditor = module(read(source / 'scripts/m7-native-product.py', 1024 * 1024).decode(), 'original_product_audit')
    credentials = auditor.identity()
    need(credentials['fsuid'] == grant['installation']['credentials']['fsuid'] == who,
         'actual service filesystem identity differs')
    initial = benchmark.native_seals(grant)
    product = Path(grant['product_dir'])
    parent = product.parent
    need(parent.resolve() == parent and product.resolve() == product, 'product parent alias')
    for directory in (parent, product):
        info = directory.lstat()
        need(stat.S_ISDIR(info.st_mode) and info.st_uid == who, 'product directory owner/type differs')
    def products():
        result = {}
        for name, expected in grant['artifacts'].items():
            path = product / name
            fd = auditor.open_regular(path)
            try:
                before = os.fstat(fd)
                metadata = holders.metadata(before)
                need(stat.S_IMODE(before.st_mode) == 0o700 and before.st_uid == credentials['fsuid']
                     and before.st_gid == grant['installation']['files'][name]['gid'], 'opened product ownership/mode differs')
                auditor.digest_fd(fd, expected)
                same(metadata, holders.metadata(os.fstat(fd)))
                same(metadata, holders.metadata(path.lstat()))
                result[name] = metadata
            finally:
                os.close(fd)
        return result
    opened_products = products()
    selected = grant['selected_engine']
    selected_files = {}
    for name, expected in selected['files'].items():
        path = Path(selected['engine_build']) / name
        info = path.lstat()
        need(stat.S_ISREG(info.st_mode) and info.st_size == expected['bytes']
             and stat.S_IMODE(info.st_mode) == expected['mode']
             and benchmark.sha(path) == expected['sha256'], 'selected engine bytes/modes differ')
        selected_files[str(path)] = dict(expected)
    need(len(selected_files) == 13 and benchmark.sha(Path(selected['engine_build']) / 'sail-build.json')
         == selected['manifest_sha256'], 'selected engine manifest differs')
    paths = set(initial['files']) | set(selected_files)
    paths.update(str(source / name) for name, row in config['inventory'].items() if row['kind'] == 'file')
    def bindings():
        rows = {}
        for name in sorted(paths):
            info = Path(name).lstat()
            need(stat.S_ISREG(info.st_mode), 'protected file changed type')
            rows[name] = {'device': info.st_dev, 'inode': info.st_ino, 'mode': info.st_mode,
                          'uid': info.st_uid, 'gid': info.st_gid, 'bytes': info.st_size,
                          'mtime_ns': info.st_mtime_ns, 'ctime_ns': info.st_ctime_ns}
            if Path(name).is_relative_to(source):
                need(info.st_uid == credentials['fsuid'], 'source file ownership differs')
        return rows
    def identity():
        leader = holders.row(1)
        need(leader is not None and leader['state'] not in ('Z', 'X'), 'PID1 unavailable')
        mounts = read('/proc/self/mountinfo', MAX_METADATA)
        return {'pid1': {key: leader[key] for key in ('pid', 'ppid', 'pgid', 'start')}, 'mountinfo_sha256': digest(mounts),
                'product_directory': holders.directory_id(product),
                'source_directory': holders.directory_id(source), 'fsuid': who}
    before = identity()
    pinned = bindings()
    aliases = [grant['product_dir'], grant['source_path'], '/work/target']
    inodes = {(row['device'], row['inode']) for row in pinned.values()}
    first = holders.holders(aliases, inodes, gates.dead_threads)
    closed(first)
    verifier.verify_source(source, inventory)
    final = benchmark.native_seals(grant)
    same(initial, final)
    for name, expected in selected_files.items():
        need(benchmark.sha(Path(name)) == expected['sha256'], 'selected runtime changed')
    same(pinned, bindings())
    same(opened_products, products())
    second = holders.holders(aliases, inodes, gates.dead_threads)
    closed(second)
    after = identity()
    same(before, after)
    return {'status': 'PASS', 'identity': after, 'source_entries': len(config['inventory']),
            'seals': final, 'selected_engine_files': selected_files, 'bindings': pinned,
            'holders_before': first, 'holders_after': second, 'credentials': credentials,
            'opened_products': opened_products, 'producer_external_target_link': config['producer'],
            'scope': 'Exact product/source/target aliases and protected inodes; no general orphan certificate'}


def host(args):
    repo = Path(__file__).resolve().parents[1]
    code = support(repo)
    gates = module(code['scripts/local-combined-gates.py'], 'transport')
    benchmark = module(code['docs/validation/m7-current-benchmark-controller.py'], 'benchmark')
    # Installer import uses its sibling only to define the original pure source verifier.
    spec = importlib.util.spec_from_file_location('original_install', repo / 'docs/validation/m7-current-benchmark-install.py')
    installer = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(installer)
    holders = module(code['scripts/inventory-old-benchmark-single-deps.py'], 'holder_metadata')
    closure = gates.Closure()
    out = Path(args.output)
    need(out.parent == Path('/tmp') and re.fullmatch('sail-local-product-rebind-' + args.revision + '-[a-zA-Z0-9_-]+', out.name),
         'new independent recovery namespace required')
    out.mkdir(mode=0o700, exist_ok=False)
    directory = Path('/tmp/sail-m7-core-product-' + args.revision)
    pointer = directory / 'product-grant-acceptance.json'
    producer_path = directory / 'product-grant.json'
    original = read(pointer)
    need(digest(original) == args.old_grant_sha256, 'old execution grant differs')
    producer_raw = read(producer_path)
    producer_sha = digest(producer_raw)
    old = benchmark.validate_grant(json.loads(original, object_pairs_hook=unique), args.revision)
    producer = benchmark.validate_grant(json.loads(producer_raw, object_pairs_hook=unique), args.revision)
    need(old['container'] == 'sail-m7-acceptance' and producer['container'] == 'sail-m7-handoff-validation', 'container roles differ')
    need(old['producer_grant_sha256'] == producer_sha, 'producer provenance differs')
    for key in ('artifacts', 'source_commit', 'source_archive_sha256'):
        same(old[key], producer[key])
    # Archive immutable historical records before any pointer update.
    (out / 'historical-execution-grant.json').write_bytes(original)
    (out / 'historical-producer-grant.json').write_bytes(producer_raw)
    archive = installer.source_archive(directory / 'source.tar', old['source_archive_sha256'])
    same(archive['inventory'], benchmark.read_json(directory / 'acceptance-install/source-inventory.json'))
    expected = json.loads(read(args.expected_containers), object_pairs_hook=unique)
    need(set(expected) == set(CONTAINERS), 'exact dual-container birth grant required')
    result = {'version': 1, 'status': 'FAIL', 'scope': 'Revalidation/rebind only; historical qualification retained',
              'old_execution_grant_sha256': digest(original), 'producer_grant_sha256': producer_sha,
              'source_commit': old['source_commit'], 'source_archive_sha256': archive['sha256'],
              'controller_sha256': digest(read(__file__)), 'support_sha256': SUPPORT,
              'source_members': archive['members'], 'commands': [], 'pointer_updated': False}
    gates.save(out / 'result.json', result)
    def command(name, argv, payload=None, limit=30):
        closure.require()
        capture = gates.bounded_command(argv, timeout=limit, closure=closure, payload=payload,
                                       capture_limit=MAX_METADATA, check=False)
        raw = capture.stdout
        (out / (name + '.json')).write_bytes(raw)
        result['commands'].append({'name': name, 'exit_code': capture.returncode,
                                   'sha256': digest(raw), 'bytes': len(raw)})
        gates.save(out / 'result.json', result)
        need(capture.returncode == 0, 'revalidation transport/helper failed: ' + name)
        return json.loads(raw, object_pairs_hook=unique)
    def inspect_all(phase):
        actual = {}
        for name in CONTAINERS:
            template = ('{"Id":{{json .Id}},"Image":{{json .Image}},"Created":{{json .Created}},'
                        '"State":{"Running":{{json .State.Running}},"Paused":{{json .State.Paused}},'
                        '"StartedAt":{{json .State.StartedAt}}},"Mounts":{{json .Mounts}}}')
            item = command(phase + '-' + name, ['docker', 'inspect', '--format', template, name])
            need(item['State']['Running'] and not item['State']['Paused'], 'container unavailable')
            actual[name] = {'Id': item['Id'], 'Image': item['Image'], 'Created': item['Created'],
                            'birth': item['State']['StartedAt'], 'Mounts': holders.canonical_mounts(item['Mounts'])}
        holders.same_inspections(actual, expected, phase)
        return actual
    try:
        before = inspect_all('before')
        for name, historical in ((CONTAINERS[0], old), (CONTAINERS[1], producer)):
            record = historical['execution_container_birth']
            need(before[name]['Id'] == record['id'] and before[name]['Image'] == record['image'],
                 'rebind only supports the same existing container/image')
        proofs = {}
        program = read(__file__).decode()
        for phase in ('before', 'after'):
            for name, grant in ((CONTAINERS[0], old), (CONTAINERS[1], producer)):
                config = {'support': code, 'grant': grant, 'revision': args.revision,
                          'inventory': archive['inventory'], 'verify_source': inspect.getsource(installer.verify_source),
                          'producer': name == CONTAINERS[1]}
                payload = json.dumps(config).encode()
                need(len(payload) <= MAX_METADATA, 'native payload bound')
                proof = command('revalidate-' + phase + '-' + name,
                                ['docker', 'exec', '-i', name, 'python3', '-c', program, '--native'], payload, 600)
                need(proof['status'] == 'PASS' and proof['source_entries'] == archive['members'], 'native inventory incomplete')
                if phase == 'after':
                    for key in ('identity', 'seals', 'selected_engine_files', 'bindings', 'credentials', 'opened_products'):
                        same(proofs[name]['before'][key], proof[key])
                proofs.setdefault(name, {})[phase] = proof
        after = inspect_all('after')
        holders.same_inspections(after, before, 'revalidation')
        need(digest(read(producer_path)) == producer_sha and digest(read(pointer)) == digest(original), 'historical grants changed')
        result.update(status='PASS', containers=after, revalidation=proofs,
                      no_build=True, no_install=True, no_proof=True, closure_uncertainty=closure.reasons)
        gates.save(out / 'revalidation.json', result)
        evidence = digest(read(out / 'revalidation.json'))
        current = after[CONTAINERS[0]]
        birth = {'id': current['Id'], 'image': current['Image'], 'created': current['Created'], 'started_at': current['birth']}
        candidate = rebound(old, producer_sha, birth, evidence, digest(original))
        benchmark.validate_grant(candidate, args.revision)
        raw = (json.dumps(candidate, indent=2) + '\n').encode()
        (out / 'rebound-execution-grant.json').write_bytes(raw)
        closure.require()
        if args.commit:
            replace_pointer(pointer, digest(original), raw)
            result.update(pointer_updated=True, current_execution_grant_sha256=digest(raw))
        gates.save(out / 'result.json', result)
        return 0
    except BaseException as error:
        result.update(status='FAIL', error_type=type(error).__name__, error=str(error)[:512], closure_uncertainty=closure.reasons)
        gates.save(out / 'result.json', result)
        return 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--native', action='store_true', help=argparse.SUPPRESS)
    parser.add_argument('--revision')
    parser.add_argument('--old-grant-sha256')
    parser.add_argument('--expected-containers', type=Path)
    parser.add_argument('--output', type=Path)
    parser.add_argument('--commit', action='store_true', help='Update only the designated current execution pointer after full PASS')
    args = parser.parse_args()
    if args.native:
        raw = sys.stdin.buffer.read(MAX_METADATA + 1)
        need(len(raw) <= MAX_METADATA, 'native metadata bound')
        print(json.dumps(native(json.loads(raw, object_pairs_hook=unique))))
        return 0
    need(args.revision and re.fullmatch('[0-9a-f]{7}', args.revision), 'exact short product revision required')
    need(args.old_grant_sha256 and re.fullmatch('[0-9a-f]{64}', args.old_grant_sha256), 'historical execution digest required')
    need(args.expected_containers and args.output, 'fresh output and exact current birth/mount binding required')
    def interrupted(number, _frame):
        raise KeyboardInterrupt('rebind interrupted: ' + str(number))
    signal.signal(signal.SIGTERM, interrupted)
    signal.signal(signal.SIGHUP, interrupted)
    return host(args)


if __name__ == '__main__':
    raise SystemExit(main())
