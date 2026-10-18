#!/usr/bin/env python3
"""Record the staged vstd proof and bind every staged release member (no publication)."""
import argparse
import contextlib
import hashlib
import json
import math
import os
from pathlib import Path, PurePosixPath
import shutil
import signal
import stat
import subprocess
import sys
import tempfile
import tarfile
import time

if sys.version_info < (3, 12):
    raise SystemExit('release-shipped: Python 3.12 or newer is required')

SCHEMA = 12
LIMIT = 4 * 1024**3
CAPTURE_LIMIT = 32 * 1024**2
INDEX = 'release-files.json'
ROOT = Path(__file__).resolve().parent.parent


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def paths(root):
    stack = [root]
    count = 0
    while stack:
        current = stack.pop()
        with os.scandir(current) as entries:
            children = sorted(entries, key=lambda entry: entry.name)
        for entry in children:
            path = Path(entry.path)
            meta = path.lstat()
            count += 1
            assert count <= 16384, 'release entry limit exceeded'
            yield path, meta
            if stat.S_ISDIR(meta.st_mode):
                stack.append(path)


def inventory(root):
    files = {}
    total = 0
    for path, meta in paths(root):
        if stat.S_ISDIR(meta.st_mode):
            continue
        assert stat.S_ISREG(meta.st_mode), 'nonregular release member: ' + str(path)
        name = str(path.relative_to(root))
        if name == INDEX:
            continue
        total += meta.st_size
        assert total <= LIMIT and len(files) < 16384, 'release member limit exceeded'
        files[name] = {'sha256': digest(path), 'bytes': meta.st_size,
                       'mode': stat.S_IMODE(meta.st_mode)}
    return files


def directories(root):
    return {str(path.relative_to(root)): stat.S_IMODE(meta.st_mode)
            for path, meta in paths(root) if stat.S_ISDIR(meta.st_mode)}


def owned_size(root, endpoints):
    total = 0
    for path in root.rglob('*'):
        try:
            meta = path.lstat()
        except FileNotFoundError:
            continue  # Cargo may remove its own temporary build files.
        if stat.S_ISREG(meta.st_mode):
            total += meta.st_size
        elif stat.S_ISDIR(meta.st_mode):
            pass
        else:
            assert (path in endpoints and stat.S_ISSOCK(meta.st_mode)
                    and meta.st_uid == os.getuid() and stat.S_IMODE(meta.st_mode) == 0o600), 'unknown owned special'
        assert total <= LIMIT, 'owned release recording limit exceeded'
    return total


def checked_name(text):
    path = PurePosixPath(text)
    assert text and not path.is_absolute() and '..' not in path.parts
    assert str(path) == text and '\\' not in text
    return path


def complete_report(doc):
    assert doc['kind'] == 'sail-project-result' and doc['schema_version'] == 2
    assert doc['currentness'] == 'current' and doc['exit_code'] == 0 and doc['error'] is None
    assert doc.get('no_reuse') is None and doc.get('engine') is None
    assert isinstance(doc['run_id'], str) and doc['run_id']
    targets = doc['targets']
    assert len(targets) == 2 and sum(t['requested'] is True for t in targets) == 1
    assert len({(t['package_id'], t['target'], tuple(t['kind'])) for t in targets}) == 2
    for target in targets:
        assert target['kind'] == ['lib']
        assert target['complete'] is True and target['reused'] is False
        assert target['establishing_run'] == doc['run_id'] and not target.get('dependency_build')
        report = target['report']
        assert report['schema_version'] == 2 and report['mode'] == 'default'
        assert report['exit_code'] == 0 and report['error'] is None
        assert report['diagnostics'] == [] and report.get('engine') is None
        summary = report['summary']
        assert all(type(summary[name]) is int for name in ('functions','verified','failed','unknown','provisional','unstable'))
        assert summary['functions'] > 0 and summary['functions'] == summary['verified']
        assert all(summary[n] == 0 for n in ('failed', 'unknown', 'provisional', 'unstable'))
        functions = [f for crate in report['crates'] for f in crate['functions']]
        assert len(functions) == summary['functions']
        assert all(f['verdict'] == 'verified' and f['establishment'] == 'confirmed'
                   and f['unstable'] is False and f['verus_success'] is True
                   and f['provenance'].get('engine') is None for f in functions)
        assert all(crate['verus']['success'] is True and crate['verus']['errors'] == 0
                   and crate['verus']['encountered_vir_error'] is False for crate in report['crates'])
    requested = [t for t in targets if t['requested']]
    assert requested[0]['target'] == 'sail_shipped_dependencies'
    assert requested[0]['report']['summary']['functions'] == 1
    vstd = [t for t in targets if not t['requested'] and t['target'] == 'vstd']
    assert len(vstd) == 1
    assert doc['execution']['verifier_invocations'] == 2 and doc['execution']['reused_targets'] == 0
    wall = doc['execution']['wall_ms']
    assert type(wall) in (int, float) and math.isfinite(wall) and wall >= 0
    return doc['run_id']


def check_bundle(bundle, variant, manifest_sha256, run=None):
    tree = inventory(bundle)
    manifest = json.loads((bundle/'manifest.json').read_bytes())
    assert manifest['schema'] == SCHEMA and manifest['kind'] == 'sail-vstd-compile-boundary'
    assert manifest['runtime'] == variant and len(manifest['identity']) == 64
    assert (bundle/'Cargo.lock').is_file() and len(manifest['units']) == 1
    expected = {'manifest.json', 'Cargo.lock'}
    for key, description in manifest['units'].items():
        assert len(key) == 64 and all(c in '0123456789abcdef' for c in key)
        assert description.startswith('vstd ')
        prefix = 'units/'+key+'/'
        expected.add(prefix+'unit.json')
        unit = json.loads((bundle/(prefix+'unit.json')).read_bytes())
        assert unit['schema'] == SCHEMA and unit['key'] == key
        assert unit['verifying'] is True and unit['crate_name'] == 'vstd'
        assert unit['source_tree'] and unit['sources']
        assert run is None or unit['establishing_run'] == run
        reads = unit['compiler_reads']
        assert reads['complete'] is True and set(reads['phases']) == {'verification', 'compilation'}
        assert all(phase['sources'] for phase in reads['phases'].values())
        members = [(prefix+'files/'+row['name'], row) for row in unit['files']]
        members += [(prefix+unit[name]['name'], unit[name]) for name in ('stdout', 'stderr')]
        for name, row in members:
            checked_name(name)
            assert name not in expected, 'duplicate stored member'
            expected.add(name)
            assert tree[name]['sha256'] == row['sha256'] and tree[name]['bytes'] == row['size']
            assert bool(tree[name]['mode'] & 0o111) == row['executable']
    assert set(tree) == expected, 'extra/missing proof bundle member'
    return {'schema': SCHEMA, 'identity': manifest['identity'], 'runtime': variant,
            'runtime_manifest_sha256': manifest_sha256, 'manifest_sha256': digest(bundle/'manifest.json'),
            'members': tree}


def verify(root):
    index = json.loads((root/INDEX).read_bytes())
    assert set(index) == {'schema', 'proof', 'files', 'directories'}
    assert index['schema'] == 1 and index['files'] == inventory(root)
    assert index['directories'] == directories(root)
    assert set(index['files']).issuperset({'bin/sail', 'bin/sail-z3'})
    bundle = root/'deps'/index['proof']['runtime']
    runtime_manifest = root/'engine'/index['proof']['runtime']/'sail-build.json'
    assert digest(runtime_manifest) == index['proof']['runtime_manifest_sha256']
    assert check_bundle(bundle, index['proof']['runtime'], digest(runtime_manifest)) == index['proof']
    return index


def retain_compiler_inputs(owned, evidence, required):
    """Retain certificates only; these diagnostics never authorize a proof."""
    destination = evidence/'compiler-inputs'
    destination.mkdir(mode=0o700)
    index = {'complete': False, 'root': str(owned), 'limit': CAPTURE_LIMIT, 'unique_bytes': 0, 'paths': []}
    (destination/'bodies').mkdir(mode=0o700)
    def save():
        (destination/'index.json').write_text(json.dumps(index, indent=2)+'\n')
    def stamp(meta):
        return (meta.st_dev, meta.st_ino, meta.st_mode, meta.st_size, meta.st_mtime_ns, meta.st_ctime_ns)
    def unreadable(error):
        raise error
    try:
        bodies = set()
        kinds = set()
        for parent, directories, files in os.walk(owned, onerror=unreadable):
            assert all(not (Path(parent)/name).is_symlink() for name in directories), 'certificate directory alias'
            for name in files:
                path = Path(parent)/name
                relative = path.relative_to(owned)
                observed = {part for part in relative.parts if part in ('.sail-boundary-inputs', '.sail-boundary-native-reads')}
                if not (name == 'inputs.json' and observed or 'skipped' in relative.parts):
                    continue
                before = path.lstat()
                assert stat.S_ISREG(before.st_mode), 'certificate alias or special file'
                assert before.st_size <= CAPTURE_LIMIT, 'certificate too large'
                fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
                with os.fdopen(fd, 'rb') as source:
                    assert stamp(os.fstat(source.fileno())) == stamp(before)
                    data = source.read(CAPTURE_LIMIT+1)
                    assert len(data) == before.st_size and stamp(os.fstat(source.fileno())) == stamp(before) == stamp(path.lstat()), 'certificate changed'
                value = hashlib.sha256(data).hexdigest()
                assert len(index['paths']) < 16384, 'certificate entry limit'
                if value not in bodies:
                    assert index['unique_bytes']+len(data) <= CAPTURE_LIMIT, 'certificate capture limit'
                    # Reserve before writing, without refund on a failed write.
                    index['unique_bytes'] += len(data)
                    path = destination/'bodies'/value
                    with path.open('xb') as stream:
                        os.chmod(path, 0o600)
                        stream.write(data)
                    bodies.add(value)
                index['paths'].append({'path': str(relative), 'sha256': value,
                                       'bytes': len(data), 'mode': stat.S_IMODE(before.st_mode)})
                if name == 'inputs.json':kinds.update(observed)
        index['paths'].sort(key=lambda row: row['path'])
        assert not required or kinds == {'.sail-boundary-inputs', '.sail-boundary-native-reads'}, 'required compiler certificates missing'
        index['complete'] = True
    finally:
        save()


def record(root, evidence):
    assert root.is_absolute() and not root.is_symlink() and not evidence.exists()
    evidence.mkdir(parents=True, mode=0o700)
    before = inventory(root)
    assert not (root/'deps').exists() and not (root/INDEX).exists(), 'stage already contains dependencies'
    binaries = root/'bin'
    env = {k: v for k, v in os.environ.items() if not k.startswith('SAIL_')
           and k not in ('CARGO_TARGET_DIR', 'CARGO_ENCODED_RUSTFLAGS', 'RUSTFLAGS', 'VERUS_Z3_PATH')}
    # The staged binary is authoritative for normal discovery; there is no compiler override.
    status = subprocess.run([str(binaries/'sail'), 'engine', 'status', '--output', 'json'],
                            env=dict(env, SAIL_CACHE_DIR=str(evidence/'status-cache')),
                            capture_output=True, timeout=30, check=True)
    (evidence/'engine-status.json').write_bytes(status.stdout)
    selected = json.loads(status.stdout)
    build = Path(selected['build'])
    assert selected['version'] == 2 and build.parent == root/'engine'
    assert selected['manifest_sha256'] == digest(build/'sail-build.json')
    variant = build.name
    end = time.monotonic()+600
    server = None
    outcome = {'passed': False, 'stage': str(root), 'variant': variant}
    def stop_server():
        if server is not None and server.poll() is None:
            os.killpg(server.pid, signal.SIGTERM)
            try:server.wait(timeout=10)
            except subprocess.TimeoutExpired:os.killpg(server.pid, signal.SIGKILL);server.wait(timeout=10)
        assert server is None or server.poll() is not None, 'owned server survived'
    try:
        with tempfile.TemporaryDirectory(prefix='sail-release-record-', dir='/tmp') as temporary, contextlib.ExitStack() as cleanup:
            owned = Path(temporary).resolve()
            def preserve_inputs():
                try:retain_compiler_inputs(owned, evidence, outcome['passed'])
                except BaseException:
                    outcome['passed'] = False
                    raise
            cleanup.callback(preserve_inputs)
            cleanup.callback(stop_server)
            (evidence/'scratch-root.json').write_text(json.dumps({
                'pid': os.getpid(), 'path': str(owned), 'uid': os.getuid()})+'\n')
            project = owned/'project'
            shutil.copytree(ROOT/'scripts/shipped-deps', project)
            home = owned/'cargo-home';home.mkdir()
            bundle = owned/'bundle';cache = owned/'cache';socket = owned/'daemon.sock'
            env.update(CARGO_HOME=str(home), SAIL_CACHE_DIR=str(cache),
                       SAIL_DAEMON_SOCKET=str(socket), SAIL_VSTD_BOUNDARY_RESEARCH='1',
                       SAIL_SHIPPED_RECORD=str(bundle),
                       SAIL_SHIPPED_TRACE=str(evidence/'boundary-trace'))
            endpoints = {socket, socket.with_name('jobs-v1-proof-v1-'+hashlib.sha256(str(socket).encode()).hexdigest()[:8]+'.sock')}
            def command(label, argv, cwd=project, cap=240):
                started = time.monotonic()
                remaining = min(cap, end-time.monotonic()-30)
                assert remaining > 0, 'release record deadline reached'
                with (evidence/(label+'.stdout')).open('xb') as out, (evidence/(label+'.stderr')).open('xb') as err:
                    child = subprocess.Popen(argv, cwd=cwd, env=env, stdout=out, stderr=err,
                                             stdin=subprocess.DEVNULL, start_new_session=True)
                    try:
                        while child.poll() is None:
                            assert time.monotonic() < end-30 and time.monotonic() < started+remaining
                            assert os.fstat(out.fileno()).st_size+os.fstat(err.fileno()).st_size <= CAPTURE_LIMIT
                            owned_size(owned, endpoints)
                            time.sleep(.2)
                        assert os.fstat(out.fileno()).st_size+os.fstat(err.fileno()).st_size <= CAPTURE_LIMIT
                        assert child.returncode == 0, label+' failed'
                    finally:
                        if child.poll() is None:
                            os.killpg(child.pid, signal.SIGTERM)
                            try:child.wait(timeout=10)
                            except subprocess.TimeoutExpired:os.killpg(child.pid, signal.SIGKILL);child.wait(timeout=10)
                return (evidence/(label+'.stdout')).read_bytes()
            command('fetch', ['cargo', 'fetch', '--locked'], cap=120)
            with (evidence/'server.stdout').open('xb') as out, (evidence/'server.stderr').open('xb') as err:
                server = subprocess.Popen([str(binaries/'sail'), 'serve', '--socket', str(socket),
                                           '--cache', str(cache/'results')], env=env,
                                          stdout=out, stderr=err, start_new_session=True)
            try:
                deadline = min(end-30,time.monotonic()+10)
                while not socket.exists():
                    assert server.poll() is None and time.monotonic()<deadline
                    time.sleep(.05)
                raw = command('record', [str(binaries/'sail'), 'verify', '--locked', '--output', 'json'])
                outcome['server_pid'] = server.pid
                run = complete_report(json.loads(raw))
                proof = check_bundle(bundle, variant, selected['manifest_sha256'], run)
            finally:
                stop_server()
            assert inventory(root) == before, 'staged source bytes changed during recording'
            # Build and check the complete private tree before it becomes discoverable.
            staging = root/('.deps-recording-'+str(os.getpid()))
            assert not staging.exists()
            try:
                shutil.copytree(bundle, staging/variant)
                assert check_bundle(staging/variant, variant, selected['manifest_sha256'], run) == proof
                staging.rename(root/'deps')
            finally:
                if staging.exists():shutil.rmtree(staging)
            index = {'schema': 1, 'proof': proof, 'files': inventory(root),
                     'directories': directories(root)}
            (root/INDEX).write_text(json.dumps(index, indent=2)+'\n')
            verify(root)
            outcome.update(passed=True, establishing_run=run, proof=proof)
    except BaseException:
        outcome['passed'] = False
        raise
    finally:
        try:
            try:
                after = inventory(root)
                outcome['original_stage_unchanged'] = all(after.get(name) == row for name, row in before.items())
                assert outcome['original_stage_unchanged'], 'original staged member changed'
            except BaseException as error:
                outcome['passed'] = False
                outcome['final_guard_error'] = repr(error)
                raise
        finally:
            try:
                if not outcome['passed']:
                    if (root/'deps').exists():shutil.rmtree(root/'deps')
                    (root/INDEX).unlink(missing_ok=True)
            finally:
                outcome['server_reaped'] = server is None or server.poll() is not None
                (evidence/'result.json').write_text(json.dumps(outcome, indent=2)+'\n')


def unpack(archive, destination):
    assert not any(destination.iterdir()), 'unpack destination must be empty'
    with tarfile.open(archive, 'r:gz') as packed:
        members = packed.getmembers()
        assert len(members) <= 16384 and sum(m.size for m in members) <= LIMIT
        names = [str(checked_name(m.name.rstrip('/'))) for m in members]
        assert len(set(names)) == len(names)
        roots = {PurePosixPath(name).parts[0] for name in names}
        assert len(roots) == 1
        top = next(iter(roots))
        assert top.startswith('sail-') and all(m.isfile() or m.isdir() for m in members)
        packed.extractall(destination, filter='data')
    verify(destination/top)
    return destination/top


def main():
    def terminated(_signum, _frame):
        raise KeyboardInterrupt('release recording terminated')
    signal.signal(signal.SIGTERM, terminated)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('mode', choices=('record', 'verify', 'unpack'))
    parser.add_argument('stage', type=Path)
    parser.add_argument('--evidence', type=Path)
    parser.add_argument('--destination', type=Path)
    args = parser.parse_args()
    os.umask(0o022)
    stage = args.stage.resolve(strict=True)
    if args.mode == 'unpack':
        assert args.destination is not None
        print(unpack(stage, args.destination.resolve(strict=True)))
    elif args.mode == 'verify':verify(stage)
    else:
        assert args.evidence is not None
        record(stage, args.evidence.resolve())


if __name__ == '__main__':
    main()
