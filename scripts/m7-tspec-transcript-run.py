#!/usr/bin/env python3
"""Run one frozen tspec logging diagnostic; preserve the harness's own policy."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import tarfile
import time

MAX_SCAN = 16384
MAX_GROUPS = 256
MAX_FILES = 4096
MAX_BYTES = 512 * 1024 * 1024


def sha(path):
    digest = hashlib.sha256()
    with path.open('rb') as file:
        for block in iter(lambda: file.read(1024 * 1024), b''):
            digest.update(block)
    return digest.hexdigest()


def save(path, value):
    path.write_text(json.dumps(value, indent=2) + '\n')


def interrupted(number, _frame):
    """Route catchable controller termination through its owned finally cleanup."""
    raise KeyboardInterrupt(f'controller signal {number}')


def row(pid):
    try:
        with open(f'/proc/{pid}/stat', 'rb') as file:
            raw = file.read(4097)
        if len(raw) > 4096:
            return None
        fields = raw.rsplit(b') ', 1)[1].split()
        return {'pid': pid, 'ppid': int(fields[1]), 'pgid': int(fields[2]),
                'start': int(fields[19]), 'state': fields[0].decode()}
    except (OSError, ValueError, IndexError):
        return None


def matching(actual, pinned):
    return (actual is not None and actual['pid'] == pinned['pid']
            and actual['start'] == pinned['start'] and actual['pgid'] == pinned['pgid'])


class Groups:
    """Retain only group leaders observed in a live pinned parent's subtree."""
    def __init__(self, leader, max_groups=MAX_GROUPS):
        assert leader['pgid'] == leader['pid']
        if type(max_groups) is not int or not 1 <= max_groups <= MAX_SCAN:
            raise ValueError('invalid bounded group budget')
        self.leader = leader
        self.known = {leader['pid']: leader}
        self.max_groups = max_groups
        self.partial = False
        self.first_refusal = None

    def mark_partial(self, reason, pid=None, pinned_start=None, observed_start=None):
        """Retain one fixed diagnostic; a later clean scan never refunds refusal."""
        self.partial = True
        if self.first_refusal is None:
            self.first_refusal = {'reason': reason, 'known_groups': len(self.known),
                                  'group_budget': self.max_groups, 'pid': pid,
                                  'pinned_start': pinned_start, 'observed_start': observed_start}

    def observe(self, rows):
        primary = rows.get(self.leader['pid'])
        if not matching(primary, self.leader) or primary['state'] in ('Z', 'X'):
            self.mark_partial('primary-unavailable-or-dead-or-rebound', self.leader['pid'],
                              self.leader['start'], None if primary is None else primary['start'])
            return
        owned = {self.leader['pid']}
        while True:
            added = {pid for pid, r in rows.items()
                     if r['ppid'] in owned and pid not in owned
                     and rows[r['ppid']]['state'] not in ('Z', 'X')
                     and r['state'] not in ('Z', 'X')
                     and r['start'] >= rows[r['ppid']]['start']}
            if not added:
                break
            owned.update(added)
        for pid in sorted(owned):
            r = rows[pid]
            if r['pid'] != r['pgid']:
                continue
            if pid not in self.known and len(self.known) >= self.max_groups:
                self.mark_partial('group-budget-exhausted', pid, observed_start=r['start'])
                continue
            previous = self.known.get(pid)
            if previous is not None and not matching(r, previous):
                self.mark_partial('known-group-birth-changed', pid, previous['start'], r['start'])
                continue
            self.known[pid] = dict(r)

    def signal_live(self, read=row, send=os.killpg):
        events = []
        # Child groups first; the harness remains present through their checks.
        for pid, pinned in reversed(list(self.known.items())):
            actual = read(pid)
            if not matching(actual, pinned) or actual['state'] in ('Z', 'X'):
                continue
            try:
                send(pid, signal.SIGKILL)
                events.append({'pid': pid, 'start': pinned['start'], 'signal': 'KILL'})
            except ProcessLookupError:
                pass
            except OSError as error:
                events.append({'pid': pid, 'start': pinned['start'],
                               'error': str(error)[:256]})
        return events


def scan(groups):
    rows = {}
    with os.scandir('/proc') as entries:
        scanned = 0
        for entry in entries:
            if not entry.name.isdecimal():
                continue
            scanned += 1
            if scanned > MAX_SCAN:
                groups.mark_partial('process-scan-budget-exhausted')
                break
            r = row(int(entry.name))
            if r is not None:
                rows[r['pid']] = r
    groups.observe(rows)


def validate(root, expected, observer, observer_sha):
    freeze = root / 'freeze.json'
    assert sha(freeze) == expected
    f = json.loads(freeze.read_text())
    assert f['status'] == 'PREPARED_NOT_EXECUTED' and not f['actor_launched']
    assert f['original_unit_timeout_seconds'] == 3600 and f['jobs'] == 1
    assert f['original_plain_runs'] == 1 and f['original_conditional_confirm_runs'] == 2
    assert f['added_verus_args'] == ['--log', 'smt-transcript', '--log-dir', '{out}/smt-transcripts']
    assert sha(observer) == observer_sha
    private = root / 'private-environment.json'
    assert sha(private) == f['private_environment_sha256']
    assert private.stat().st_uid == 0 and private.stat().st_mode & 0o777 == 0o600
    env = json.loads(private.read_text())
    assert env['SAIL_UNIT_TIMEOUT'] == '3600' and env['HOME'] == '/root'
    assert not env.get('XDG_CACHE_HOME')
    assert {k for k in env if k.startswith('SAIL_')} == {'SAIL_VERUS_HOME', 'SAIL_TEST_Z3', 'SAIL_UNIT_TIMEOUT'}
    assert env['SAIL_VERUS_HOME'] == '/opt/verus' and env['SAIL_TEST_Z3'] == '/opt/verus/verus-x86-linux/z3'
    assert all(not env.get(k) for k in ['LD_PRELOAD', 'ROSETTA_DEBUGSERVER_PORT', 'VERUS_Z3_PATH'])
    cwd = Path(f['cwd'])
    assert sha(cwd / 'corpus/corpus.toml') == f['diagnostic_corpus_sha256']
    assert sha(cwd / 'corpus/toolchain.toml') == f['toolchain_sha256']
    for name, seal in f['artifacts'].items():
        path = Path(name)
        assert path.stat().st_size == seal['bytes'] and sha(path) == seal['sha256']
    for tree in f['source_trees'].values():
        actual_names = set()
        for directory, dirs, names in os.walk(tree['root']):
            dirs[:] = [n for n in dirs if n != '.git']
            for name in names:
                if name == '.sail-fetched':
                    continue
                path = Path(directory) / name
                assert not path.is_symlink() and path.is_file()
                actual_names.add(str(path.relative_to(tree['root'])))
                assert len(actual_names) <= 10000
        assert actual_names == set(tree['files']), 'source inventory changed'
        head = subprocess.run(['git', '-C', tree['root'], 'rev-parse', 'HEAD'],
                              capture_output=True, check=True).stdout.decode().strip()
        assert head == tree['commit']
        for name, seal in tree['files'].items():
            path = Path(tree['root']) / name
            assert path.is_file() and not path.is_symlink()
            assert path.stat().st_size == seal['bytes'] and sha(path) == seal['sha256']
    return f, env


class ArchiveRead:
    def __init__(self, file, deadline):
        self.file = file
        self.deadline = deadline

    def read(self, count):
        if time.monotonic() >= self.deadline:
            raise TimeoutError('supplementary archive deadline')
        return self.file.read(count)


def archive(root):
    """Bound only the supplementary copy; preserve every original retained file."""
    deadline = time.monotonic() + 120
    result = {'complete': True, 'files': 0, 'bytes': 0, 'private_environment_exported': False}
    destination = root / 'supplementary.tar'
    excluded = {'private-environment.json', 'supplementary.tar', 'archive.json'}
    try:
        with tarfile.open(destination, 'x') as output:
            for directory, dirs, names in os.walk(root):
                if time.monotonic() >= deadline:
                    raise TimeoutError('supplementary archive deadline')
                dirs[:] = [n for n in dirs if not (Path(directory) / n).is_symlink()]
                for name in names:
                    path = Path(directory) / name
                    if path.parent == root and name in excluded:
                        continue
                    if path.is_symlink() or not path.is_file():
                        result['complete'] = False
                        continue
                    size = path.stat().st_size
                    if result['files'] >= MAX_FILES or result['bytes'] + size > MAX_BYTES:
                        raise ValueError('supplementary archive file/byte cap')
                    member = output.gettarinfo(str(path), str(path.relative_to(root)))
                    with path.open('rb') as file:
                        output.addfile(member, ArchiveRead(file, deadline))
                    result['files'] += 1
                    result['bytes'] += size
    except (OSError, ValueError, TimeoutError) as error:
        result.update(complete=False, reason=str(error)[:256])
    result['archive_sha256'] = sha(destination)
    result['original_files_deleted'] = False
    save(root / 'archive.json', result)
    return result


def run(args):
    assert os.getuid() == 0 and args.root.is_absolute()
    f, env = validate(args.root, args.freeze_sha, args.observer, args.observer_sha)
    root = args.root
    assert not (root / 'actor.json').exists() and not (root / 'harness').exists()
    assert os.statvfs(root).f_bavail * os.statvfs(root).f_frsize >= 2 * 1024 ** 3
    record = {'version': 1, 'scope': 'One selected-unit logging diagnostic; no acceptance credit',
              'status': 'STARTING', 'controller_sha256': sha(Path(__file__)),
              'freeze_sha256': args.freeze_sha, 'original_unit_seconds': 3600,
              'manual_retry': False, 'outer_proof_timeout_added': False,
              'observer_sha256': args.observer_sha, 'cleanup': []}
    process = observer = groups = None
    started = time.monotonic()
    try:
        with (root / 'stdout').open('xb') as out, (root / 'stderr').open('xb') as err:
            process = subprocess.Popen(f['argv'], cwd=f['cwd'], env=env, stdin=subprocess.DEVNULL,
                                       stdout=out, stderr=err, start_new_session=True)
            pinned = row(process.pid)
            assert pinned is not None and pinned['pid'] == pinned['pgid']
            groups = Groups(pinned)
            record.update(status='RUNNING', actor=pinned, started_unix_ns=time.time_ns())
            save(root / 'actor.json', record)
            with (root / 'observer.stdout').open('xb') as obsout, (root / 'observer.stderr').open('xb') as obserr:
                observer = subprocess.Popen(['python3', str(args.observer), '--pid', str(process.pid),
                    '--start-time', str(pinned['start']), '--output', str(root / 'observation.jsonl')],
                    stdin=subprocess.DEVNULL, stdout=obsout, stderr=obserr)
                while process.poll() is None:
                    scan(groups)
                    try:
                        process.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        pass
            record.update(status='TERMINAL', exit_code=process.returncode,
                          elapsed_seconds=time.monotonic() - started)
    except BaseException as error:
        record.update(status='CONTROLLER_FAILURE', error_type=type(error).__name__, error=str(error)[:256])
        raise
    finally:
        # Normal harness terminal remains authoritative even if supplementary
        # cleanup cannot certify an escaped or unobserved descendant.
        if groups is not None:
            record['cleanup'] = groups.signal_live()
            record['retained_groups'] = list(groups.known.values())
            record['ownership_partial'] = groups.partial
            record['ownership_group_budget'] = groups.max_groups
            record['ownership_first_refusal'] = groups.first_refusal
        elif process is not None and process.poll() is None:
            # A failed initial identity capture grants only the Popen child,
            # not any guessed descendant group.
            process.kill()
        if process is not None:
            try:
                process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                record['cleanup_error'] = 'owned primary did not exit after KILL'
        if observer is not None:
            if observer.poll() is None:
                observer.terminate()
            try:
                observer.wait(timeout=5)
            except subprocess.TimeoutExpired:
                observer.kill()
                observer.wait(timeout=5)
            record['observer_exit_code'] = observer.returncode
        record['orphan_absence_certified'] = False
        save(root / 'actor.json', record)
    for name, seal in f['artifacts'].items():
        assert sha(Path(name)) == seal['sha256'], 'post-run runtime changed'
    validate(root, args.freeze_sha, args.observer, args.observer_sha)
    record['runtime_seals_unchanged'] = True
    record['logging_is_not_accepted_wire'] = True
    record['archive'] = archive(root)
    save(root / 'terminal.json', record)
    print(json.dumps({'status': record['status'], 'exit_code': record.get('exit_code'),
                      'acceptance': False, 'elapsed_seconds': record.get('elapsed_seconds')}))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, required=True)
    parser.add_argument('--freeze-sha', required=True)
    parser.add_argument('--observer', type=Path, required=True)
    parser.add_argument('--observer-sha', required=True)
    parser.add_argument('--validate-only', action='store_true')
    args = parser.parse_args()
    if args.validate_only:
        validate(args.root, args.freeze_sha, args.observer, args.observer_sha)
        print(json.dumps({'preflight': 'PASS', 'actor_launched': False,
                          'controller_sha256': sha(Path(__file__))}))
    else:
        for number in (signal.SIGTERM, signal.SIGHUP):
            signal.signal(number, interrupted)
        run(args)
