#!/usr/bin/env python3
"""Record the staged vstd proof and bind every staged release member (no publication)."""
import argparse
import contextlib
from collections import Counter
import hashlib
import importlib.util
import json
import math
import os
import re
from pathlib import Path, PurePosixPath
import shutil
import signal
import stat
import subprocess
import sys
import tempfile
import tarfile
import time
import tomllib

if sys.version_info < (3, 12):
    raise SystemExit('release-shipped: Python 3.12 or newer is required')
if not __debug__:
    raise SystemExit('release-shipped: required validation needs Python without -O or PYTHONOPTIMIZE')

SCHEMA = 12
LIMIT = 4 * 1024**3
CAPTURE_LIMIT = 32 * 1024**2
SOURCE_LIMIT = 128 * 1024**2
INDEX = 'release-files.json'
RLIMIT_WITNESS = 'rlimit.json'
WITNESS_RLIMIT = '100'
# Cargo's progress lines, whose order follows Cargo's parallel scheduling.
CARGO_PROGRESS = re.compile(r'^\s*(Compiling|Checking|Fresh|Finished|Running|Locking|Blocking|Downloaded|Downloading) ')
REPLIES = 'replies'
PACK_MAGIC = b'sail-reply-pack-v1\n'
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


def inventory(root, limit=LIMIT):
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
        assert total <= limit and len(files) < 16384, 'release member limit exceeded'
        files[name] = {'sha256': digest(path), 'bytes': meta.st_size,
                       'mode': stat.S_IMODE(meta.st_mode)}
    return files


def directories(root):
    return {str(path.relative_to(root)): stat.S_IMODE(meta.st_mode)
            for path, meta in paths(root) if stat.S_ISDIR(meta.st_mode)}


def owned_endpoints(socket):
    """Exact cache, jobs and runtime addresses owned by this recorder's daemon."""
    jobs = socket.with_name('jobs-v1-proof-v1-'+hashlib.sha256(os.fsencode(socket)).hexdigest()[:8]+'.sock')
    runtime = jobs.with_name('rt-'+hashlib.sha256(os.fsencode(jobs)).hexdigest()[:16])/'v1.sock'
    return {socket, jobs, runtime}


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


def rust_source(build, env=None):
    """Check proof-bundle prerequisites; this is not proof/key authority."""
    pin = tomllib.loads((ROOT/'corpus/toolchain.toml').read_text())['verus']['rust_toolchain']
    assert json.loads((build/'sail-build.json').read_bytes())['rust_toolchain'] == pin
    advice = ('proof bundles require complete pinned rust-src; run '
              'rustup component add rust-src --toolchain '+pin)
    selected = subprocess.run(['rustup', 'run', pin, 'rustc', '--print', 'sysroot'],
                              env=env, capture_output=True, timeout=30, check=True)
    sysroot = Path(selected.stdout.decode().strip())
    assert sysroot.is_absolute() and sysroot.resolve(strict=True) == sysroot, advice
    root = sysroot/'lib/rustlib/src/rust'
    try:
        assert root.resolve(strict=True) == root, advice
        components = sysroot/'lib/rustlib/components'
        manifest = sysroot/'lib/rustlib/manifest-rust-src'
        for path in (components, manifest):
            assert stat.S_ISREG(path.lstat().st_mode) and path.stat().st_size <= 4 * 1024**2, advice
        installed = set(components.read_text().splitlines())
        assert 'rust-src' in installed, advice
        # The proof binds every installed compiler library, so it is recorded
        # from the README's toolchain: the minimal profile plus rust-src.
        hosts = [name[len('rustc-'):] for name in installed if name.startswith('rustc-')]
        assert len(hosts) == 1 and installed == {'rust-src', *(f'{c}-{hosts[0]}' for c in ('cargo', 'rustc', 'rust-std'))}, (
            'proof bundles are recorded from the minimal pinned toolchain with rust-src only; installed: '
            + ' '.join(sorted(installed)))
        expected = set()
        for line in manifest.read_text().splitlines():
            prefix = 'file:lib/rustlib/src/rust/'
            assert line.startswith(prefix), advice
            name = str(checked_name(line[len(prefix):]))
            assert name not in expected, advice
            expected.add(name)
            assert len(expected) <= 16384, advice
        actual = inventory(root, limit=SOURCE_LIMIT)
        assert expected and set(actual) == expected, advice
        total = sum(row['bytes'] for row in actual.values())
        assert total <= SOURCE_LIMIT, advice
    except (FileNotFoundError, OSError) as error:
        raise ValueError(advice) from error
    return {'toolchain': pin, 'sysroot': str(sysroot), 'source_root': str(root),
            'source_files': len(actual), 'source_bytes': total,
            'prerequisite_inventory_sha256': hashlib.sha256(
                json.dumps(actual, sort_keys=True).encode()).hexdigest(),
            'scope': 'installed component completeness only; serving still requires fresh source/key/certificate admission'}


def verified_report(report):
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
        verified_report(target['report'])
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
    assert (bundle/'Cargo.lock').is_file() and 1 <= len(manifest['units']) <= 2
    if isinstance(run, dict):
        assert set(run) == set(manifest['units']), 'missing per-profile source run'
    elif run is not None:
        assert len(manifest['units']) == 1, 'multiple profiles require separate source runs'
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
        expected_run = run[key] if isinstance(run, dict) else run
        assert expected_run is None or unit['establishing_run'] == expected_run
        reads = unit['compiler_reads']
        assert reads['complete'] is True and set(reads['phases']) == {'verification', 'compilation'}
        assert all(phase['sources'] for phase in reads['phases'].values())
        if (prefix+RLIMIT_WITNESS) in tree:
            witness = json.loads((bundle/(prefix+RLIMIT_WITNESS)).read_bytes())
            assert witness['schema'] == 1 and witness['kind'] == 'sail-rlimit-witness' and witness['key'] == key
            expected.add(prefix+RLIMIT_WITNESS)
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


def recorded_rlimits(unit):
    """The exact forwarded limits in this recorded compiler environment."""
    tokens = [token for name, value in unit.get('environment', {}).items()
              if name.startswith(' __VERUS_DRIVER_ARGS')
              for token in value.split('__VERUS_DRIVER_ARGS_SEP__')]
    limits = []
    for i, token in enumerate(tokens):
        if token == '--rlimit':
            assert i+1 < len(tokens), 'missing recorded resource limit'
            limits.append(tokens[i+1])
        elif token.startswith('--rlimit='):
            limits.append(token.partition('=')[2])
    return limits


def join_exact_profiles(bundle, raised, variant, manifest_sha256, original_run, raised_run):
    """Join two independently confirmed proofs, preserving their exact keys."""
    check_bundle(bundle, variant, manifest_sha256, original_run)
    check_bundle(raised, variant, manifest_sha256, raised_run)
    original = json.loads((bundle/'manifest.json').read_bytes())
    additional = json.loads((raised/'manifest.json').read_bytes())
    for field in ('schema', 'kind', 'runtime', 'identity', 'virtual_base'):
        assert original[field] == additional[field], 'incompatible proof profiles'
    assert (bundle/'Cargo.lock').read_bytes() == (raised/'Cargo.lock').read_bytes()
    [original_key], [raised_key] = original['units'], additional['units']
    assert original_key != raised_key, 'raised profile reused the default key'
    for directory, key, expected in ((bundle, original_key, []), (raised, raised_key, [WITNESS_RLIMIT])):
        unit = json.loads((directory/'units'/key/'unit.json').read_bytes())
        assert recorded_rlimits(unit) == expected, 'unexpected recorded resource-limit profile'
    shutil.copytree(raised/'units'/raised_key, bundle/'units'/raised_key)
    original['units'].update(additional['units'])
    (bundle/'manifest.json').write_text(json.dumps(original, indent=2)+'\n')
    runs = {original_key: original_run, raised_key: raised_run}
    return check_bundle(bundle, variant, manifest_sha256, runs), runs


def check_replies(replies, report=None):
    """The bundled vstd replies (ADR 0062): exactly a pack and its public key."""
    assert sorted(path.name for path in replies.iterdir()) == ['release.pub', 'vstd.pack']
    pack = (replies/'vstd.pack').read_bytes()
    assert pack.startswith(PACK_MAGIC), 'malformed reply pack'
    at, count, previous = len(PACK_MAGIC), 0, b''
    while at < len(pack):
        key, length = pack[at:at+32], int.from_bytes(pack[at+32:at+36], 'little')
        assert len(key) == 32 and key > previous and at+36+length <= len(pack), 'malformed reply pack'
        previous, at, count = key, at+36+length, count+1
    assert at == len(pack) and count > 0, 'empty or malformed reply pack'
    public = (replies/'release.pub').read_text()
    assert public.startswith('sail-ed25519-public-v1 ') and len(public.split()) == 2
    if report is not None:
        assert report['publish']['published'] == count and report['publish']['failed'] == 0
        assert report['pack_bytes'] == len(pack)
    return {'entries': count, 'pack_sha256': digest(replies/'vstd.pack'),
            'key_sha256': digest(replies/'release.pub')}


def drain_queue(binaries, socket, env):
    """Wait (bounded) until the recording daemon has written every reply."""
    deadline = time.monotonic()+30
    while time.monotonic() < deadline:
        status = subprocess.run([str(binaries/'sail'), 'status', '--json', '--socket', str(socket)],
                                env=env, capture_output=True, timeout=10)
        if status.returncode == 0 and json.loads(status.stdout)['info']['queue']['entries'] == 0:
            return
        time.sleep(.1)
    raise AssertionError('recording daemon did not drain its write queue')


def verify(root):
    index = json.loads((root/INDEX).read_bytes())
    assert set(index) == {'schema', 'proof', 'files', 'directories'}
    assert index['schema'] == 1 and index['files'] == inventory(root)
    assert index['directories'] == directories(root)
    assert set(index['files']).issuperset({'bin/sail', 'bin/sail-z3'})
    # Only the internal temporal-enabled variant (ADR 0083) has the native
    # worker, and then always with its exact installation manifest.
    worker_files = {'bin/sail-runtime-worker', 'bin/sail-runtime-worker.json'}
    assert not (worker_files & set(index['files'])) or worker_files <= set(index['files'])
    if worker_files <= set(index['files']):
        worker = json.loads((root/'bin/sail-runtime-worker.json').read_bytes())
        assert worker['version'] == 1 and worker['protocol'] == 1
        assert worker['sha256'] == digest(root/'bin/sail-runtime-worker')
    assert set(index['files']).issuperset({REPLIES+'/vstd.pack', REPLIES+'/release.pub'})
    check_replies(root/REPLIES)
    bundle = root/'deps'/index['proof']['runtime']
    runtime_manifest = root/'engine'/index['proof']['runtime']/'sail-build.json'
    assert digest(runtime_manifest) == index['proof']['runtime_manifest_sha256']
    assert check_bundle(bundle, index['proof']['runtime'], digest(runtime_manifest)) == index['proof']
    return index


def first_use(root, doc):
    """Refuse SAFE_MISS: a proof-bundle smoke must actually serve its release."""
    index = verify(root)
    proof = index['proof']
    bundle = root/'deps'/proof['runtime']
    manifest = json.loads((bundle/'manifest.json').read_bytes())
    defaults = [key for key in manifest['units']
                if not recorded_rlimits(json.loads((bundle/'units'/key/'unit.json').read_bytes()))]
    assert len(defaults) == 1, 'first use needs one exact default-limit profile'
    identity = proof['identity']+'/'+defaults[0]
    origin = {'kind': 'release', 'identity': identity, 'establishing_run': 'shipped:'+identity}
    assert doc['kind'] == 'sail-project-result' and doc['schema_version'] == 2
    assert doc['currentness'] == 'current' and doc['exit_code'] == 0 and doc['error'] is None
    assert doc.get('no_reuse') is None and doc.get('engine') is None
    assert isinstance(doc['run_id'], str) and doc['run_id']
    targets = doc['targets']
    assert len(targets) == 2 and len({(t['package_id'], t['target'], tuple(t['kind'])) for t in targets}) == 2
    for target in targets:
        assert target['complete'] is True and target['reused'] is False
        assert target['kind'] == ['lib'] and target.get('dependency_store') is None
        verified_report(target['report'])
    vstd = [t for t in targets if t['target'] == 'vstd' and t['requested'] is False]
    requested = [t for t in targets if t['requested'] is True]
    assert len(vstd) == len(requested) == 1
    assert vstd[0].get('dependency_build') == origin, 'first use did not serve the admitted release proof'
    assert vstd[0]['establishing_run'] == origin['establishing_run']
    assert requested[0].get('dependency_build') is None and requested[0]['establishing_run'] == doc['run_id']
    execution = doc['execution']
    assert type(execution['verifier_invocations']) is int and execution['verifier_invocations'] == 1
    assert type(execution['reused_targets']) is int and execution['reused_targets'] == 0
    assert type(execution['wall_ms']) in (int, float) and math.isfinite(execution['wall_ms']) and execution['wall_ms'] >= 0
    return identity


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


def process_census():
    """Bounded PID/group/state view; prerequisite before any owned-group launch."""
    try:
        raw = subprocess.run(['ps', '-axo', 'pid=,pgid=,stat='],
                             capture_output=True, check=True, timeout=2).stdout
    except (OSError, subprocess.SubprocessError) as error:
        raise RuntimeError('release-shipped: owned cleanup requires ps (procps on Linux)') from error
    assert len(raw) <= 8*1024**2, 'process census limit exceeded'
    rows = {}
    for line in raw.splitlines():
        pid, group, state = line.split()
        pid, group = int(pid), int(group)
        assert pid > 0 and group >= 0 and state and pid not in rows, 'invalid process census'
        rows[pid] = (group, state)
    assert rows[os.getpid()][0] == os.getpgrp(), 'process census omits caller'
    return rows


class OwnedGroup:
    """Hold the waitable session leader until its owned group has no live work."""

    def __init__(self, child, label, outcomes):
        self.child = child
        self.pid = child.pid
        self.state = {'label': label, 'pid': self.pid, 'extinct': False, 'signals': []}
        outcomes.append(self.state)

    @classmethod
    @contextlib.contextmanager
    def running(cls, argv, label, outcomes, **kwargs):
        group = None
        pending = []
        try:
            # Defer cancellation across Popen's return/ownership assignment.
            handlers = {sig: signal.signal(sig, lambda signum, _frame: pending.append(signum))
                        for sig in (signal.SIGTERM, signal.SIGINT)}
            try:
                group = cls(subprocess.Popen(argv, start_new_session=True, **kwargs), label, outcomes)
            finally:
                for sig, handler in handlers.items():signal.signal(sig, handler)
            if pending:raise KeyboardInterrupt('release child creation cancelled')
            group.members()
            yield group
        finally:
            if group is not None:group.stop()

    def members(self, deadline=None):
        if deadline is None:deadline = time.monotonic()+2
        # Never poll/wait before extinction: the unreaped PID prevents group-ID reuse.
        assert self.child.returncode is None, 'owned leader was reaped early'
        assert self.pid not in (os.getpgrp(), os.getsid(0))
        # macOS ps "sess" is not a portable decimal SID. Inspect only PGID/state;
        # use the kernel's getsid/getpgid for ownership before signalling.
        members = {}
        for pid, (group, state) in process_census().items():
            if group != self.pid:
                continue
            zombie = state.startswith(b'Z')
            if not zombie:
                try:
                    assert os.getpgid(pid) == self.pid and os.getsid(pid) == self.pid, 'foreign group member'
                except ProcessLookupError:
                    if pid == self.pid:
                        assert time.monotonic() < deadline, 'leader exit census deadline'
                        return self.members(deadline)
                    continue
            # macOS getsid/getpgid reject zombies. The waitable direct child
            # still pins this PID/group; zombies cannot change session or fork.
            members[pid] = zombie
        assert self.pid in members, 'held leader missing from process census'
        return members

    def exited(self):
        return self.members()[self.pid]

    def send(self, signum):
        assert not self.state['extinct'], 'owned group already extinct'
        if all(self.members().values()):
            return
        os.killpg(self.pid, signum)
        self.state['signals'].append(signum.name)

    def stop(self):
        if self.state['extinct']:
            return
        # A second cancellation must not interrupt the bounded cleanup already
        # unwinding the first one. Restore the caller's handlers afterwards.
        handlers = {sig: signal.signal(sig, signal.SIG_IGN)
                    for sig in (signal.SIGTERM, signal.SIGINT)}
        try:
            for signum in (signal.SIGTERM, signal.SIGKILL):
                members = self.members()
                if all(members.values()):
                    break
                self.send(signum)
                deadline = time.monotonic()+10
                while not all((members := self.members()).values()) and time.monotonic() < deadline:
                    time.sleep(.05)
            assert all(members.values()), 'owned process group survived cleanup'
            # Zombies cannot do more work or fork. The direct leader remains
            # pinned until now; after this wait the group is never signalled again.
            self.state['zombie_pids'] = sorted(members)
            self.state['returncode'] = self.child.wait(timeout=2)
            self.state['extinct'] = True
        except BaseException as error:
            self.state['cleanup_error'] = repr(error)
            raise
        finally:
            for sig, handler in handlers.items():
                signal.signal(sig, handler)


def rlimit_tool():
    spec = importlib.util.spec_from_file_location('rlimit_subsumption', ROOT/'scripts/rlimit-subsumption.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def rlimit_witness(build, owned, env, evidence, key, run):
    """ADR 0060: verify the recipe as recorded and with a raised --rlimit, with
    the staged runtime's own cargo-verus and Z3 and a recording solver wrapper.
    Returns the witness for units/<key>/rlimit.json, or None if any solver
    process, answer, resource count or output differs beyond the rlimit."""
    tool = rlimit_tool()
    release = build/'verus/source/target-verus/release'
    work = owned/'rlimit-witness'
    shutil.copytree(ROOT/'scripts/shipped-deps', work/'project')
    tee = work/'tee-z3.sh'
    tee.write_text(tool.TEE)
    tee.chmod(0o755)
    runs = {}
    for label, extra in (('default', []), ('raised', ['--', '--rlimit', WITNESS_RLIMIT])):
        capture = work/label
        capture.mkdir()
        witness_env = {k: v for k, v in env.items() if not k.startswith('SAIL_')}
        witness_env.update(PATH=str(release)+os.pathsep+env.get('PATH', ''), VERUS_Z3_PATH=str(tee),
                           SAIL_RLIMIT_Z3=str(release/'z3'), SAIL_RLIMIT_DIR=str(capture),
                           CARGO_TARGET_DIR=str(work/('target-'+label)))
        stdout, stderr = run('rlimit-'+label, [str(release/'cargo-verus'), 'verify', '--locked', *extra],
                             work/'project', witness_env)
        progress = lambda text: '\n'.join(line for line in text.splitlines() if not CARGO_PROGRESS.match(line))
        runs[label] = {'stdout': stdout.decode(errors='replace'),
                       'stderr': progress(stderr.decode(errors='replace')),
                       'streams': tool.solver_streams(str(capture))}
    default, raised = runs['default'], runs['raised']
    shape = lambda run: sorted((s[0], s[1], s[2]) for s in run['streams'])
    answers = [answer for s in default['streams'] for answer in s[2][0]]
    queries = lambda run: [pair for s in run['streams'] for pair in s[6]]
    usual = lambda run: max(Counter(limit for _, limit in queries(run) if limit).items(),
                            key=lambda item: (item[1], -item[0]), default=(0, 0))[0]
    recorded_limit, witness_limit = usual(default), usual(raised)
    largest = max(queries(default), default=(0, 0))
    result = {
        'schema': 1, 'kind': 'sail-rlimit-witness', 'key': key,
        'recorded_rlimit': float(WITNESS_RLIMIT)*recorded_limit/witness_limit if witness_limit else 0.0,
        'witness_rlimit': float(WITNESS_RLIMIT),
        'recorded_solver_limit': recorded_limit, 'witness_solver_limit': witness_limit,
        'max_query': largest[0], 'max_query_limit': largest[1],
        'over_limit_queries': sum(1 for used, limit in queries(default) if limit and used >= limit),
        'queries': len(answers), 'unknown_answers': answers.count(b'unknown'),
        'solver_processes': len(default['streams']),
        'inputs_equal': [(s[0], s[1]) for s in shape(default)] == [(s[0], s[1]) for s in shape(raised)],
        'answers_and_counts_equal': shape(default) == shape(raised),
        'normalized_inputs_sha256': hashlib.sha256(json.dumps([(s[0], s[1]) for s in shape(default)]).encode()).hexdigest(),
    }
    outputs_equal = default['stdout'] == raised['stdout'] and default['stderr'] == raised['stderr']
    (evidence/'rlimit-witness.json').write_text(json.dumps(dict(result, outputs_equal=outputs_equal), indent=2)+'\n')
    admitted = (outputs_equal and result['inputs_equal'] and result['answers_and_counts_equal']
                and result['unknown_answers'] == 0 and result['queries'] > 0
                and result['over_limit_queries'] == 0 and len(queries(default)) == result['queries']
                and 0 < result['max_query'] < result['max_query_limit']
                and 0 < recorded_limit < witness_limit)
    return result if admitted else None


def record(root, evidence):
    assert root.is_absolute() and not root.is_symlink() and not evidence.exists()
    evidence.mkdir(parents=True, mode=0o700)
    before = inventory(root)
    assert not (root/'deps').exists() and not (root/INDEX).exists(), 'stage already contains dependencies'
    assert not (root/REPLIES).exists(), 'stage already contains replies'
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
    server_group = None
    outcome = {'passed': False, 'stage': str(root), 'variant': variant, 'process_groups': []}
    def stop_server():
        if server_group is not None:
            server_group.stop()
    try:
        process_census()
        prerequisite = rust_source(build, env)
        # Cargo itself changes compiler arguments between versions. Record with
        # the same pinned toolchain as a minimal installed-runtime consumer.
        env['RUSTUP_TOOLCHAIN'] = prerequisite['toolchain']
        (evidence/'rust-source-prerequisite.json').write_text(json.dumps(prerequisite, indent=2)+'\n')
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
            endpoints = owned_endpoints(socket)
            def command(label, argv, cwd=project, cap=240, command_env=None):
                started = time.monotonic()
                remaining = min(cap, end-time.monotonic()-30)
                assert remaining > 0, 'release record deadline reached'
                with (evidence/(label+'.stdout')).open('xb') as out, (evidence/(label+'.stderr')).open('xb') as err:
                    with OwnedGroup.running(argv, label, outcome['process_groups'], cwd=cwd, env=command_env or env,
                                            stdout=out, stderr=err, stdin=subprocess.DEVNULL) as group:
                        try:
                            while not group.exited():
                                assert time.monotonic() < end-30 and time.monotonic() < started+remaining
                                assert os.fstat(out.fileno()).st_size+os.fstat(err.fileno()).st_size <= CAPTURE_LIMIT
                                owned_size(owned, endpoints)
                                time.sleep(.2)
                            assert os.fstat(out.fileno()).st_size+os.fstat(err.fileno()).st_size <= CAPTURE_LIMIT
                        except BaseException as error:
                            group.state['command_error'] = repr(error)
                            raise
                    assert group.child.returncode == 0, label+' failed'
                return (evidence/(label+'.stdout')).read_bytes()
            command('fetch', ['cargo', 'fetch', '--locked'], cap=120)
            with (evidence/'server.stdout').open('xb') as out, (evidence/'server.stderr').open('xb') as err:
                server_group = cleanup.enter_context(OwnedGroup.running(
                    [str(binaries/'sail'), 'serve', '--socket', str(socket), '--cache', str(cache/'results')],
                    'server', outcome['process_groups'], env=env, stdout=out, stderr=err))
            try:
                deadline = min(end-30,time.monotonic()+10)
                while not socket.exists():
                    assert not server_group.exited() and time.monotonic()<deadline
                    time.sleep(.05)
                raw = command('record', [str(binaries/'sail'), 'verify', '--locked', '--output', 'json'])
                outcome['server_pid'] = server_group.pid
                run = complete_report(json.loads(raw))
                # Cargo inherits the factory's umask. Bind the portable archive
                # mode before checking and indexing the owned proof bundle.
                lock = bundle/'Cargo.lock'
                assert lock.is_file() and not lock.is_symlink()
                lock.chmod(0o644)
                proof = check_bundle(bundle, variant, selected['manifest_sha256'], run)
                # ADR 0060: the unit may also serve a raised --rlimit if the
                # recipe's solver processes are unchanged apart from the limit.
                end += 600
                def witness_command(label, argv, cwd, command_env):
                    command(label, argv, cwd=cwd, cap=600, command_env=command_env)
                    return ((evidence/(label+'.stdout')).read_bytes(), (evidence/(label+'.stderr')).read_bytes())
                [unit_key] = json.loads((bundle/'manifest.json').read_bytes())['units']
                # The witness is optional: any failure leaves the unit exact-key only.
                try:
                    if sys.platform == 'darwin':
                        # The pinned macOS solver has a captured counterexample
                        # to stable resource counts. Use separately recorded
                        # exact profiles until that solver is requalified.
                        witness = None
                        outcome['rlimit_witness_refusal'] = 'pinned macOS solver resource-count variation'
                    else:
                        witness = rlimit_witness(build, owned, env, evidence, unit_key, witness_command)
                except Exception as error:
                    witness = None
                    outcome['rlimit_witness_error'] = repr(error)
                if witness is not None:
                    (bundle/'units'/unit_key/RLIMIT_WITNESS).write_text(json.dumps(witness, indent=2)+'\n')
                    (bundle/'units'/unit_key/RLIMIT_WITNESS).chmod(0o644)
                    proof = check_bundle(bundle, variant, selected['manifest_sha256'], run)
                outcome['rlimit_witness'] = witness is not None
                runs = run
                if witness is None:
                    # The witness is optional, but the common raised limit
                    # still gets its own fully verified, exact-key proof.
                    for label in ('default', 'raised'):
                        target = owned/'rlimit-witness'/('target-'+label)
                        if target.exists():shutil.rmtree(target)
                    raised_bundle = owned/'bundle-raised'
                    raised_env = dict(env, SAIL_SHIPPED_RECORD=str(raised_bundle))
                    raw = command('record-raised', [str(binaries/'sail'), 'verify', '--locked',
                                                   '--output', 'json', '--', '--rlimit', WITNESS_RLIMIT],
                                  command_env=raised_env)
                    raised_run = complete_report(json.loads(raw))
                    lock = raised_bundle/'Cargo.lock'
                    assert lock.is_file() and not lock.is_symlink()
                    lock.chmod(0o644)
                    proof, runs = join_exact_profiles(bundle, raised_bundle, variant,
                                                     selected['manifest_sha256'], run, raised_run)
                    outcome['exact_raised_profile'] = True
                    outcome['profile_establishing_runs'] = runs
                drain_queue(binaries, socket, env)
            finally:
                stop_server()
            # ADR 0062: sign the recording's solver replies with a one-off key
            # that exists only inside this command, and ship them with its
            # public half. They serve byte-identical vstd queries after a
            # shipped-proof miss; every entry is still verified when served. Pack
            # after every recorded profile. The plain witness uses its own
            # tee and pinned solver, so it cannot add cache entries.
            replies_raw = command('bundle-replies', [str(binaries/'sail'), 'cache', 'bundle-replies',
                                                     '--cache', str(cache/'results'), '--out', str(owned/'replies')],
                                  cwd=owned, cap=120)
            replies = check_replies(owned/'replies', json.loads(replies_raw))
            outcome['replies'] = replies
            assert inventory(root) == before, 'staged source bytes changed during recording'
            # Build and check the complete private tree before it becomes discoverable.
            staging = root/('.deps-recording-'+str(os.getpid()))
            assert not staging.exists()
            try:
                shutil.copytree(bundle, staging/variant)
                assert check_bundle(staging/variant, variant, selected['manifest_sha256'], runs) == proof
                shutil.copytree(owned/'replies', staging/'.replies')
                assert check_replies(staging/'.replies') == replies
                staging.rename(root/'deps')
                (root/'deps'/'.replies').rename(root/REPLIES)
            finally:
                if staging.exists():shutil.rmtree(staging)
            index = {'schema': 1, 'proof': proof, 'files': inventory(root),
                     'directories': directories(root)}
            (root/INDEX).write_text(json.dumps(index, indent=2)+'\n')
            verify(root)
            outcome.update(passed=True, establishing_run=run, proof=proof)
    except BaseException as error:
        outcome['passed'] = False
        outcome['error'] = repr(error)
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
                    if (root/REPLIES).exists():shutil.rmtree(root/REPLIES)
                    (root/INDEX).unlink(missing_ok=True)
            finally:
                outcome['server_reaped'] = all(group['extinct'] for group in outcome['process_groups']
                                               if group['label'] == 'server')
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
    parser.add_argument('mode', choices=('record', 'verify', 'unpack', 'prerequisites', 'first-use'))
    parser.add_argument('stage', type=Path)
    parser.add_argument('--evidence', type=Path)
    parser.add_argument('--destination', type=Path)
    parser.add_argument('--report', type=Path)
    args = parser.parse_args()
    os.umask(0o022)
    stage = args.stage.resolve(strict=True)
    if args.mode == 'unpack':
        assert args.destination is not None
        print(unpack(stage, args.destination.resolve(strict=True)))
    elif args.mode == 'verify':verify(stage)
    elif args.mode == 'prerequisites':
        index = verify(stage)
        print(json.dumps(rust_source(stage/'engine'/index['proof']['runtime']), indent=2))
    elif args.mode == 'first-use':
        assert args.report is not None
        print(first_use(stage, json.loads(args.report.read_bytes())))
    else:
        assert args.evidence is not None
        record(stage, args.evidence.resolve())


if __name__ == '__main__':
    main()
