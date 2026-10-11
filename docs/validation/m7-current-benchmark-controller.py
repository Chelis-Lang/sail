#!/usr/bin/env python3
"""Run one unchanged three-round repair benchmark against a sealed product."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import stat
import subprocess
import sys
import time
import types

BUNDLE = Path('engine/verus/candidates/preparation-namespace/no-original-range/valid-memo-correction/ordinary')
ORIGINAL = BUNDLE / 'repair-benchmark/run-original.py'
ORIGINAL_SHA = '477508a145aad95f7370e2f928cce034cbf8c0f4f18854fe8442a8829a154e8c'


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha(path):
    value = hashlib.sha256()
    with Path(path).open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            value.update(block)
    return value.hexdigest()


def pairs(items):
    result = {}
    for key, value in items:
        require(key not in result, 'Duplicate JSON key')
        result[key] = value
    return result


def read_json(path):
    with Path(path).open('rb') as stream:
        data = stream.read(1024 * 1024 + 1)
    require(len(data) <= 1024 * 1024, 'JSON metadata exceeds 1 MiB')
    return json.loads(data, object_pairs_hook=pairs)


def checkpoint(path, value):
    temporary = Path(path).with_suffix('.tmp')
    temporary.write_text(json.dumps(value, indent=2) + '\n')
    temporary.replace(path)


def validate_grant(grant, revision):
    require(grant['version'] == 1, 'Unsupported product record version')
    require(re.fullmatch('[0-9a-f]{7,40}', revision), 'Invalid revision')
    commit = grant['source_commit']
    require(re.fullmatch('[0-9a-f]{40}', commit) and commit.startswith(revision), 'Product revision differs')
    require(re.fullmatch('[0-9a-f]{64}', grant['source_archive_sha256']), 'Invalid archive identity')
    require(grant['container'] in ('sail-m7-handoff-validation', 'sail-m7-acceptance'), 'Unexpected container')
    short = commit[:7]
    require(grant['source_path'] == '/tmp/sail-m7-core-product-source-' + short, 'Unexpected source path')
    require(grant['product_dir'] == '/opt/sail-m7-core-product-' + short, 'Unexpected product path')
    phases = {row['phase']: row for row in grant['phases']}
    require(len(phases) == len(grant['phases']), 'Duplicate product phase')
    require(all(phases[name]['exit_code'] == 0 for name in ('release-build', 'owned-install')), 'Product build/install incomplete')
    require(grant['installation_only'] is True, 'Unexpected product record')
    install = grant['installation']
    require(install['authority'] == 'installation-only' and install['source_modified'] is False, 'Invalid installation')
    require(install['directory_mode'] == '0700', 'Installation directory is not private')
    require(set(grant['artifacts']) == set(install['files']) == {'sail', 'sail-z3'}, 'Incomplete product inventory')
    for name, artifact in grant['artifacts'].items():
        require(type(artifact['bytes']) is int and artifact['bytes'] > 0, 'Invalid artifact size')
        require(re.fullmatch('[0-9a-f]{64}', artifact['sha256']), 'Invalid artifact digest')
        file = install['files'][name]
        require(all(file[key] == artifact[key] for key in ('bytes', 'sha256')), 'Installation bytes differ')
        require(file['mode'] == '0700' and file['uid'] == install['credentials']['fsuid'], 'Installation ownership differs')
    return grant


def execution_grant(directory, revision):
    """Prefer the acceptance execution record, bound to its unchanged producer."""
    directory = Path(directory)
    producer_path = directory / 'product-grant.json'
    producer = validate_grant(read_json(producer_path), revision)
    path = directory / 'product-grant-acceptance.json'
    if not path.exists():
        return producer_path, producer
    execution = validate_grant(read_json(path), revision)
    require(execution['container'] == 'sail-m7-acceptance', 'Unexpected execution container')
    require(execution['producer_grant_sha256'] == sha(producer_path), 'Producer record changed')
    for key in ('source_commit', 'source_archive_sha256', 'artifacts'):
        require(execution[key] == producer[key], 'Execution/producer identity differs')
    require(execution['installation']['credentials']['fsuid'] == execution['execution_fsuid'], 'Execution service identity differs')
    return path, execution


def original_module(repo):
    path = Path(repo) / ORIGINAL
    require(sha(path) == ORIGINAL_SHA, 'Original wrapper changed')
    # Execute the sealed definition without import's incidental __pycache__
    # write into the otherwise byte-identical source extraction.
    module = types.ModuleType('original_benchmark')
    module.__file__ = str(path)
    exec(compile(path.read_bytes(), str(path), 'exec'), module.__dict__)
    return module


def native_seals(grant):
    repo = Path(grant['source_path'])
    product = Path(grant['product_dir'])
    require(stat.S_IMODE(product.stat().st_mode) == 0o700 and not product.is_symlink(), 'Product directory is not private')
    require(os.geteuid() == grant['installation']['credentials']['fsuid'], 'Service identity differs from installation')
    original = original_module(repo)
    qualified = read_json(repo / BUNDLE / 'qualification-result.json')
    delivery = read_json(repo / BUNDLE / 'packaging/result.json')
    require(qualified['status'] == delivery['status'] == 'PASS', 'Source/delivery incomplete')
    require(qualified['stage_counts']['passed'] == 13 and qualified['stage_counts']['not_attempted'] == 0, 'Source qualification incomplete')
    require(qualified['manifest_sha256'] == delivery['manifest_sha256'] == original.MANIFEST, 'Runtime manifest mismatch')
    require(qualified['patch_sha256'] == delivery['patch_sha256'] == original.PATCH, 'Runtime patch mismatch')
    require(sha(original.BUILD / 'sail-build.json') == original.MANIFEST, 'Runtime manifest changed')
    manifest = read_json(original.BUILD / 'sail-build.json')
    require(len(manifest['artifacts']) == 13 and len(qualified['source8_sha256']) == 8, 'Incomplete runtime/source inventory')
    require(manifest['features'] == ['sail-interactive:' + original.PATCH, 'host:x86_64-unknown-linux-gnu', 'source-lock:' + original.LOCK], 'Runtime features differ')
    modes = {row['name']: row['mode'] for row in delivery['members']}
    files = {}
    runtime = original.BUILD / 'verus/source/target-verus/release'
    require({str(p.relative_to(original.BUILD)) for p in runtime.iterdir() if p.is_file()} == set(manifest['artifacts']), 'Runtime direct inventory differs')
    expected = {original.BUILD / name: digest for name, digest in manifest['artifacts'].items()}
    expected.update({original.BUILD / 'verus/source' / name: digest for name, digest in qualified['source8_sha256'].items()})
    expected[original.BUILD / 'verus/source/Cargo.lock'] = original.LOCK
    expected.update({repo / name: digest for name, digest in original.HASHES.items()})
    expected.update({original.M6: '69288597f5f5c0d0a81a66b00a6b0f174ba17bf6a878447050a2d7f75c0c36d8', original.M6.with_name('sail-z3'): 'aaa7e3c43ee6282b575b615ad87a4c964bae14c2f6a56787c98f03d1159582bd'})
    expected.update({Path(grant['product_dir']) / name: artifact['sha256'] for name, artifact in grant['artifacts'].items()})
    for path, digest in expected.items():
        metadata = path.lstat()
        require(stat.S_ISREG(metadata.st_mode) and sha(path) == digest, 'Input bytes/type differ: ' + str(path))
        files[str(path)] = {'sha256': digest, 'bytes': metadata.st_size, 'uid': metadata.st_uid, 'gid': metadata.st_gid, 'mode': format(stat.S_IMODE(metadata.st_mode), '04o')}
    for name in manifest['artifacts']:
        require(stat.S_IMODE((original.BUILD / name).stat().st_mode) == modes[name], 'Runtime mode differs')
    for name, artifact in grant['artifacts'].items():
        actual = files[str(Path(grant['product_dir']) / name)]
        require(all(actual[key] == grant['installation']['files'][name][key] for key in ('bytes', 'uid', 'gid', 'mode')), 'Product metadata differs')
    catalogue = read_json(repo / 'corpus/repair-edits.json')
    require(catalogue['minimum_rounds'] == 3 and catalogue['minimum_median_speedup'] == 2, 'Original thresholds differ')
    corpus = {}
    for project in catalogue['projects']:
        path = Path('/root/.cache/sail/corpus') / (project['id'] + '-' + project['commit'][:12])
        commit = subprocess.check_output(['git', '-C', str(path), 'rev-parse', 'HEAD'], text=True, timeout=5).strip()
        require(commit == project['commit'], 'Corpus pin differs')
        corpus[project['id']] = commit
    return {'files': files, 'corpus': corpus, 'wrapper_sha256': ORIGINAL_SHA, 'manifest_sha256': original.MANIFEST}


def process_identity(pid):
    # Linux stat comm can contain parentheses/spaces. Never read process env.
    fields = Path('/proc', str(pid), 'stat').read_text().rsplit(')', 1)[1].split()
    return {'pid': pid, 'state': fields[0], 'ppid': int(fields[1]), 'pgid': int(fields[2]), 'start_ticks': int(fields[19])}


def native_execute(grant, output):
    output = Path(output)
    require(os.getpriority(os.PRIO_PROCESS, 0) == 0, 'Nonordinary priority')
    # This identity precedes exec of the unchanged wrapper, retaining PID/birth.
    if os.getsid(0) != os.getpid():
        os.setsid()
    checkpoint(output / 'actor.json', process_identity(os.getpid()))
    command = ['python3', str(Path(grant['source_path']) / ORIGINAL), '--repo', grant['source_path'], '--m7', grant['product_dir'] + '/sail', '--m7-sha256', grant['artifacts']['sail']['sha256'], '--m7-shim-sha256', grant['artifacts']['sail-z3']['sha256'], '--product-commit', grant['source_commit'], '--work', str(output / 'work')]
    checkpoint(output / 'argv.json', command)
    os.execvp(command[0], command)


def closure(output):
    """Bounded, read-only candidate census, not an unlimited orphan certificate."""
    started = time.monotonic()
    actor = read_json(Path(output) / 'actor.json')
    result = {'scope': 'Bounded post-cleanup metadata census; no environment or signals', 'candidates': [], 'unreadable': [], 'complete': True, 'scanned': 0}
    for path in Path('/proc').iterdir():
        if not path.name.isdigit():
            continue
        if result['scanned'] >= 8192 or time.monotonic() - started >= 1:
            result['complete'] = False
            break
        result['scanned'] += 1
        pid = int(path.name)
        try:
            before = process_identity(pid)
            if before['state'] == 'Z':
                continue
            with (path / 'cmdline').open('rb') as stream:
                command = stream.read(65536)
            matched = str(output).encode() in command or before['pgid'] == actor['pgid']
            try:
                matched |= os.readlink(path / 'cwd').startswith(str(output) + '/')
            except FileNotFoundError:
                continue
            for index, fd in enumerate((path / 'fd').iterdir()):
                if index >= 64:
                    # A capped FD scan cannot establish general absence.
                    result['complete'] = False
                    break
                try:
                    matched |= os.readlink(fd).startswith(str(output) + '/')
                except FileNotFoundError:
                    continue
            after = process_identity(pid)
            if before['start_ticks'] != after['start_ticks']:
                result['complete'] = False
            elif matched:
                result['candidates'].append(after)
        except FileNotFoundError:
            continue
        except (OSError, ValueError) as error:
            result['unreadable'].append({'pid': pid, 'error_type': type(error).__name__})
    result['elapsed_seconds'] = time.monotonic() - started
    result['complete'] &= not result['unreadable']
    return result


def docker(container, *args, timeout=120):
    return subprocess.check_output(['docker', 'exec', container, *args], timeout=timeout, text=True)


def container_identity(container):
    data = json.loads(subprocess.check_output(['docker', 'inspect', container], text=True, timeout=15))[0]
    require(data['State']['Running'] and not data['State']['Paused'], 'Container unavailable')
    return {'id': data['Id'], 'created': data['Created'], 'started_at': data['State']['StartedAt'], 'image': data['Image']}


def output_path(revision, precheck_only, run_id=None):
    require(re.fullmatch('[0-9a-f]{7}', revision), 'Use the product short revision')
    suffix = ''
    if run_id is not None:
        require(re.fullmatch('[a-z0-9][a-z0-9-]{0,31}', run_id), 'Invalid fresh run identifier')
        suffix = '-' + run_id
    return Path('/tmp/sail-m7-current-benchmark-' + revision + suffix + ('-precheck' if precheck_only else ''))


def host(revision, precheck_only, run_id=None):
    require(re.fullmatch('[0-9a-f]{7}', revision), 'Use the product short revision')
    grant_path, grant = execution_grant(Path('/tmp/sail-m7-core-product-' + revision), revision)
    output = output_path(revision, precheck_only, run_id)
    output.mkdir(mode=0o700, exist_ok=False)
    native = str(output)
    container = grant['container']
    record = {'version': 1, 'status': 'running', 'scope': 'Read-only input precheck' if precheck_only else 'One original three-round whole-loop benchmark; readers separate', 'grant_sha256': sha(grant_path), 'controller_sha256': sha(__file__), 'source_commit': grant['source_commit'], 'source_archive_sha256': grant['source_archive_sha256'], 'phases': [], 'started_unix_ns': time.time_ns(), 'original_rounds': 3, 'original_invocation_seconds': 600, 'original_minimum_speedup_each_project': 2}
    actor = None
    record['run_id'] = run_id
    base = ['python3', native + '/launcher.py', '--native-grant', native + '/product-grant.json', '--native-output', native]
    started = time.monotonic()
    checkpoint(output / 'controller.json', record)

    def interrupted(signum, frame):
        raise KeyboardInterrupt('Owned host benchmark controller interrupted')

    signal.signal(signal.SIGTERM, interrupted)
    signal.signal(signal.SIGHUP, interrupted)
    try:
        record['container_before'] = container_identity(container)
        if 'execution_container_birth' in grant:
            require(record['container_before'] == grant['execution_container_birth'], 'Execution installation container birth changed')
        archive = grant_path.with_name('source.tar')
        require(sha(archive) == grant['source_archive_sha256'], 'Product source archive differs')
        subprocess.run(['docker', 'exec', container, 'mkdir', '-m', '700', native], check=True, timeout=15)
        for path, name in ((Path(__file__), 'launcher.py'), (grant_path, 'product-grant.json')):
            subprocess.run(['docker', 'cp', str(path), container + ':' + native + '/' + name], check=True, timeout=30)
        require(docker(container, 'sha256sum', native + '/launcher.py').split()[0] == record['controller_sha256'], 'Native launcher bytes differ')
        require(docker(container, 'sha256sum', native + '/product-grant.json').split()[0] == record['grant_sha256'], 'Native product record differs')
        record['before'] = json.loads(docker(container, *base, '--native-mode', 'seals'))
        require(not json.loads(docker(container, 'python3', '-c', 'import os,json;print(json.dumps(os.path.lexists(' + repr(native + '/work') + ')))')), 'Work path already exists')
        if precheck_only:
            record['status'] = 'PRECHECK_PASS'
        else:
            command = ['docker', 'exec', container, *base, '--native-mode', 'execute']
            record['command'] = command
            with (output / 'stdout').open('wb') as stdout, (output / 'stderr').open('wb') as stderr:
                actor = subprocess.Popen(command, stdout=stdout, stderr=stderr)
                checkpoint(output / 'controller.json', record)
                code = actor.wait()
            record['exit_code'] = code
            record['status'] = 'PASS' if code == 0 else 'FAIL'
    except BaseException as error:
        record.update(status='FAIL', error_type=type(error).__name__, error=str(error))
        if actor is not None and actor.poll() is None:
            # Signal only the identity recorded before native exec. Original
            # wrapper owns phase/job cleanup; never signal a guessed descendant.
            try:
                record['interruption'] = json.loads(docker(container, *base, '--native-mode', 'interrupt', timeout=15))
                actor.wait(timeout=45)
            except BaseException as cleanup_error:
                record['cleanup_error_type'] = type(cleanup_error).__name__
    finally:
        record['elapsed_seconds'] = time.monotonic() - started
        checkpoint(output / 'primary.json', record)
        for name, operation in (('after', 'seals'), ('closure', 'closure')):
            if name == 'closure' and (actor is None):
                continue
            try:
                record[name] = json.loads(docker(container, *base, '--native-mode', operation))
                if name == 'after':
                    require(record[name] == record.get('before'), 'Post-run input seals differ')
                else:
                    record['closure_complete'] = record[name]['complete'] and not record[name]['candidates']
                    # Metadata coverage is reported independently. A capped scan
                    # of unrelated processes cannot become a quiet-host gate.
            except BaseException as error:
                record[name + '_error'] = {'type': type(error).__name__, 'message': str(error)}
                record['status'] = 'FAIL'
        try:
            record['container_after'] = container_identity(container)
            require(record['container_after'] == record.get('container_before'), 'Container birth changed')
            # Keep original native work/caches untouched. Archive raw public
            # invocation streams and controllers, not duplicate compiler caches.
            archive_command = ['docker', 'exec', container, 'tar', '-C', native, '--exclude=./work/scratch', '--exclude=./work/results/cache-m6', '--exclude=./work/results/cache-m7', '-czf', '-', '.']
            record['archive_exclusions'] = ['work/scratch', 'work/results/cache-m6', 'work/results/cache-m7']
            record['native_work_preserved'] = True
            with (output / 'native-raw.tar.gz').open('wb') as archive, (output / 'archive.stderr').open('wb') as errors:
                subprocess.run(archive_command, stdout=archive, stderr=errors, check=True, timeout=120)
            record['raw_archive'] = {'bytes': (output / 'native-raw.tar.gz').stat().st_size, 'sha256': sha(output / 'native-raw.tar.gz')}
        except BaseException as error:
            record['archive_error'] = {'type': type(error).__name__, 'message': str(error)}
            record['status'] = 'FAIL'
        checkpoint(output / 'controller.json', record)
        print(json.dumps({'status': record['status'], 'record': str(output / 'controller.json')}), flush=True)
    return 0 if record['status'] in ('PASS', 'PRECHECK_PASS') else 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('revision', nargs='?')
    parser.add_argument('--precheck-only', action='store_true')
    parser.add_argument('--run-id', help='Fresh isolated run namespace; never reuses caches, work or sockets')
    parser.add_argument('--native-grant', type=Path)
    parser.add_argument('--native-output', type=Path)
    parser.add_argument('--native-mode', choices=('seals', 'execute', 'closure', 'interrupt'))
    args = parser.parse_args()
    if args.native_mode:
        grant = read_json(args.native_grant)
        validate_grant(grant, grant['source_commit'][:7])
        if args.native_mode == 'seals':
            print(json.dumps(native_seals(grant)))
        elif args.native_mode == 'execute':
            native_execute(grant, args.native_output)
        elif args.native_mode == 'closure':
            print(json.dumps(closure(args.native_output)))
        else:
            owner = read_json(args.native_output / 'actor.json')
            current = process_identity(owner['pid'])
            require(current['start_ticks'] == owner['start_ticks'] and current['pgid'] == owner['pgid'], 'Native controller birth differs')
            os.kill(owner['pid'], signal.SIGTERM)
            print(json.dumps({'signal': 'SIGTERM', 'owner': owner}))
        return 0
    require(args.revision is not None, 'Revision required')
    return host(args.revision, args.precheck_only, args.run_id)


if __name__ == '__main__':
    raise SystemExit(main())
