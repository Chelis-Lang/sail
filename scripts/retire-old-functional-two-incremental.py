#!/usr/bin/env python3
"""Inventory all old functional projects; retire scratch in two fixed incremental trees."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import runpy
import signal
import stat
import subprocess
import sys
import time
import types

sys.dont_write_bytecode = True
CONTAINERS = ('sail-m7-acceptance', 'sail-m7-handoff-validation')
BASE = '/tmp/sail-m7-retained-functional-1e-real'
SCOPE = BASE + '/cache/projects'
ROOTS = (SCOPE,)
SCRATCH_RELATIVE = ('eaec40e0c3f85be4088de7e277655ec2d95236310ce9a35d3a6e55f191dc93ba/7a647bc547e72b80f194a67b9a8e9ce5c1526960cc3137e620dff38df63f9a62/target/debug/incremental', 'eaec40e0c3f85be4088de7e277655ec2d95236310ce9a35d3a6e55f191dc93ba/1ea2dd8353b95b7a7a201f86fac6ede364b4fb1ebd4b4ec50bc2e44fe60f41c5/target/debug/incremental')
SCRATCH_ROOTS = tuple(SCOPE + '/' + part for part in SCRATCH_RELATIVE)
MAX_FILES = 20000
MAX_BYTES = 8 * 1024 ** 3
MAX_METADATA = 16 * 1024 ** 2
SUFFIXES = ('.o', '.rmeta', '.rlib', '.bin', '.lock')


def need(ok, message):
    if not ok:
        raise ValueError(message)


def save(path, value):
    temporary = path.with_suffix(path.suffix + '.new')
    with temporary.open('x') as handle:
        json.dump(value, handle, indent=2)
        handle.write('\n')
    temporary.replace(path)


def load(path):
    with path.open('rb') as handle:
        raw = handle.read(MAX_METADATA + 1)
    need(len(raw) <= MAX_METADATA, 'metadata limit')
    return json.loads(raw)


def digest(path):
    value = hashlib.sha256()
    with path.open('rb') as handle:
        for data in iter(lambda: handle.read(1024 * 1024), b''):
            value.update(data)
    return value.hexdigest()


def metadata(info):
    return {'device': info.st_dev, 'inode': info.st_ino, 'mode': info.st_mode,
            'bytes': info.st_size, 'links': info.st_nlink,
            'mtime_ns': info.st_mtime_ns, 'ctime_ns': info.st_ctime_ns,
            'allocated_bytes': info.st_blocks * 512}


def directory_id(path):
    info = path.lstat()
    need(stat.S_ISDIR(info.st_mode), 'directory is missing, linked or special')
    return {key: metadata(info)[key] for key in ('device', 'inode', 'mode')}


def admitted_incremental(relative):
    """Only descendants of two immutable old incremental roots."""
    return (not relative.is_absolute() and bool(relative.parts)
            and '..' not in relative.parts
            and any(relative.parts[:len(Path(prefix).parts)] == Path(prefix).parts
                    and len(relative.parts) > len(Path(prefix).parts)
                    for prefix in SCRATCH_RELATIVE))


def admitted_file(relative, info, magic):
    return (admitted_incremental(relative) and stat.S_ISREG(info.st_mode)
            and not info.st_mode & 0o111 and info.st_nlink == 1
            and magic != b'\x7fELF' and (relative.name.endswith(SUFFIXES) or relative.name == 'lock'))


def validate_inventory(value):
    """An approval file cannot expand the fixed namespace or file predicate."""
    need(value.get('roots') == list(ROOTS) and set(value['root_ids']) == set(ROOTS), 'approval root scope differs')
    entries = value['entries']
    candidates = value['candidates']
    need(isinstance(entries, dict) and len(entries) <= MAX_FILES and isinstance(candidates, list)
         and len(candidates) == len(set(candidates)), 'approval inventory shape differs')
    for name, entry in entries.items():
        root = entry.get('root')
        relative = Path(entry.get('relative', ''))
        need(root in ROOTS and not relative.is_absolute() and relative.parts
             and '..' not in relative.parts and str(Path(root) / relative) == name,
             'approval entry escapes fixed root')
    for name in candidates:
        entry = entries.get(name)
        need(isinstance(entry, dict) and entry.get('kind') == 'file', 'approval candidate absent')
        info = types.SimpleNamespace(st_mode=entry['mode'], st_nlink=entry['links'])
        need(admitted_file(Path(entry['relative']), info, b''), 'approval candidate predicate differs')
    return value


def open_regular(path):
    fd = os.open(path, os.O_RDONLY | os.O_CLOEXEC | os.O_NOFOLLOW | os.O_NONBLOCK)
    if not stat.S_ISREG(os.fstat(fd).st_mode):
        os.close(fd)
        raise ValueError('not a regular file')
    return fd


def inventory(roots=ROOTS):
    """Hash every file, link and directory; keep all but admitted scratch."""
    result = {'roots': list(roots), 'root_ids': {}, 'entries': {}, 'candidates': [],
              'candidate_allocated_bytes': 0, 'regular_bytes': 0,
              'protected_hardlinked_files': 0}
    deadline = time.monotonic() + 300
    for raw in roots:
        root = Path(raw)
        # No symlink in any prefix of the immutable admitted root.
        for prefix in reversed((root,) + tuple(root.parents)):
            directory_id(prefix)
        result['root_ids'][raw] = directory_id(root)
        pending = [root]
        while pending:
            parent = pending.pop()
            with os.scandir(parent) as entries:
                for entry in entries:
                    need(time.monotonic() < deadline, 'inventory time limit')
                    need(len(result['entries']) < MAX_FILES, 'inventory entry limit')
                    path = Path(entry.path)
                    relative = path.relative_to(root)
                    info = path.lstat()
                    key = str(path)
                    row = {'root': raw, 'relative': str(relative), **metadata(info)}
                    if stat.S_ISDIR(info.st_mode):
                        row = {'root': raw, 'relative': str(relative), 'kind': 'directory',
                               **directory_id(path)}
                        pending.append(path)
                    elif stat.S_ISLNK(info.st_mode):
                        need(not admitted_incremental(relative), 'candidate namespace symlink')
                        row.update(kind='symlink', target=os.readlink(path))
                    elif stat.S_ISREG(info.st_mode):
                        result['regular_bytes'] += info.st_size
                        need(result['regular_bytes'] <= MAX_BYTES, 'inventory byte limit')
                        fd = open_regular(path)
                        try:
                            before = os.fstat(fd)
                            need(metadata(before) == metadata(info), 'opened identity changed')
                            value = hashlib.sha256()
                            with os.fdopen(fd, 'rb', closefd=False) as stream:
                                magic = stream.read(4)
                                value.update(magic)
                                for data in iter(lambda: stream.read(1024 * 1024), b''):
                                    need(time.monotonic() < deadline, 'inventory time limit')
                                    value.update(data)
                            need(metadata(os.fstat(fd)) == metadata(before) == metadata(path.lstat()),
                                 'file changed during inventory')
                            row.update(kind='file', sha256=value.hexdigest())
                            # A linked inode may have members outside both owned
                            # cache roots. Hash and preserve it; do not infer
                            # ownership or count its blocks as reclaimable.
                            if info.st_nlink > 1:
                                row['protected_reason'] = 'hardlinked regular file'
                                result['protected_hardlinked_files'] += 1
                            if admitted_file(relative, info, magic):
                                result['candidates'].append(key)
                                result['candidate_allocated_bytes'] += info.st_blocks * 512
                        finally:
                            os.close(fd)
                    else:
                        need(False, 'unknown cache file type')
                    result['entries'][key] = row
    result['candidates'].sort()
    result['entries'] = dict(sorted(result['entries'].items()))
    return result


def row(pid):
    try:
        with open('/proc/' + str(pid) + '/stat', 'rb') as handle:
            raw = handle.read(4097)
        need(len(raw) <= 4096, 'stat limit')
        fields = raw.rsplit(b') ', 1)[1].split()
        return {'pid': pid, 'ppid': int(fields[1]), 'pgid': int(fields[2]),
                'start': int(fields[19]), 'state': fields[0].decode()}
    except (OSError, ValueError, IndexError):
        return None


def below(value, roots):
    return any(value == root or value.startswith(root + '/') for root in roots)


def holders(aliases, inodes, guard):
    """Metadata only, no environment reads; inability to inspect refuses."""
    result = {'holders': [], 'unavailable': [], 'terminated_unreaped': [],
              'self_excluded': [], 'scanned': 0, 'complete': True}
    own = row(os.getpid())
    need(own is not None, 'self birth unavailable')
    remaining = 64 * 1024 ** 2
    deadline = time.monotonic() + 30
    def read(path, maximum):
        nonlocal remaining
        need(time.monotonic() < deadline and remaining > 0, 'holder budget')
        with path.open('rb') as handle:
            value = handle.read(min(maximum, remaining) + 1)
        need(len(value) <= min(maximum, remaining), 'holder bytes exceeded')
        remaining -= len(value)
        return value
    with os.scandir('/proc') as entries:
        for entry in entries:
            if not entry.name.isdecimal():
                continue
            result['scanned'] += 1
            need(result['scanned'] <= 8192 and time.monotonic() < deadline, 'holder census limit')
            pid = int(entry.name)
            before = row(pid)
            if before is None:
                result['unavailable'].append({'pid': pid, 'reason': 'birth unreadable'})
                continue
            if pid == own['pid']:
                after = row(pid)
                if after is not None and (before['pid'], before['start']) == (own['pid'], own['start']) == (after['pid'], after['start']) and all(value['state'] in ('R', 'S', 'D', 'T', 't', 'W', 'I', 'K', 'P') for value in (own, before, after)):
                    result['self_excluded'].append(after)
                    continue
            if before['state'] in ('Z', 'X'):
                evidence = guard(types.SimpleNamespace(row=row), before)
                if evidence.get('complete') is True and evidence.get('status') == 'terminated':
                    result['terminated_unreaped'].append(dict(before, evidence=evidence))
                else:
                    result['unavailable'].append({'pid': pid, 'reason': 'task closure unavailable'})
                continue
            if before['state'] not in ('R', 'S', 'D', 'T', 't', 'W', 'I', 'K', 'P'):
                result['unavailable'].append({'pid': pid, 'reason': 'unknown task state'})
                continue
            process = Path('/proc') / str(pid)
            reasons = []
            try:
                command = read(process / 'cmdline', 1024 ** 2)
                if any(root.encode() in command for root in aliases):
                    reasons.append('command')
                for name in ('cwd', 'exe'):
                    if below(os.readlink(process / name), aliases):
                        reasons.append(name)
                maps = read(process / 'maps', 2 * 1024 ** 2)
                for line in maps.splitlines():
                    fields = line.split(None, 5)
                    if len(fields) < 5:
                        need(False, 'maps row malformed')
                    device = fields[3].split(b':')
                    need(len(device) == 2, 'maps device malformed')
                    identity = (os.makedev(int(device[0], 16), int(device[1], 16)), int(fields[4]))
                    if identity in inodes or len(fields) == 6 and below(os.fsdecode(fields[5]).removesuffix(' (deleted)'), aliases):
                        reasons.append('mapping')
                with os.scandir(process / 'fd') as descriptors:
                    for count, descriptor in enumerate(descriptors):
                        need(count < 4096 and time.monotonic() < deadline, 'descriptor limit')
                        path = Path(descriptor.path)
                        target = os.readlink(path)
                        need(len(os.fsencode(target)) <= 4096, 'descriptor path limit')
                        info = path.stat()
                        if (info.st_dev, info.st_ino) in inodes or below(target.removesuffix(' (deleted)'), aliases):
                            reasons.append('descriptor')
                after = row(pid)
                need(after is not None and all(after[k] == before[k] for k in ('pid', 'pgid', 'start')) and after['state'] in ('R', 'S', 'D', 'T', 't', 'W', 'I', 'K', 'P'), 'process birth/state unavailable/changed')
                if reasons:
                    result['holders'].append(dict(before, reasons=sorted(set(reasons))))
            except (OSError, ValueError) as error:
                result['unavailable'].append({'pid': pid, 'reason': str(error)[:128]})
    result['complete'] = not result['holders'] and not result['unavailable'] and time.monotonic() < deadline
    return result


def compare_preserved(before, after, removed):
    need(before['root_ids'] == after['root_ids'], 'root identity changed')
    need(set(after['entries']) == set(before['entries']) - set(removed), 'membership changed')
    need(all(after['entries'][name] == value for name, value in before['entries'].items()
             if name not in removed), 'preserved cache bytes/metadata changed')


def aliases(inspections):
    """Map the exact roots through explicit mounts; refuse ambiguous storage."""
    result = {name: list(ROOTS) for name in CONTAINERS}
    mappings = []
    for root in ROOTS:
        mounts = [m for m in inspections[CONTAINERS[0]].get('Mounts', [])
                  if below(root, [m['Destination']])]
        if not mounts:
            mappings.append({'root': root, 'storage': 'private-container-rootfs'})
            continue
        selected = max(mounts, key=lambda m: len(m['Destination']))
        need(selected.get('Type') in ('bind', 'volume') and selected.get('Source'), 'unknown root mount')
        underlying = selected['Source'].rstrip('/') + root[len(selected['Destination'].rstrip('/')):]
        mappings.append({'root': root, 'storage': selected['Type'], 'source': underlying})
        for name in CONTAINERS:
            for mount in inspections[name].get('Mounts', []):
                source = mount.get('Source', '').rstrip('/')
                if source and below(underlying, [source]):
                    result[name].append(mount['Destination'].rstrip('/') + underlying[len(source):])
    return {'aliases': {k: sorted(set(v)) for k, v in result.items()}, 'storage': mappings}


def canonical_mounts(records):
    """Retain every mount field; array ordering supplies no storage authority."""
    need(isinstance(records, list), 'mount records unavailable')
    destinations = set()
    for item in records:
        need(isinstance(item, dict) and isinstance(item.get('Destination'), str)
             and item['Destination'].startswith('/'), 'mount destination unavailable')
        need(item['Destination'] not in destinations, 'duplicate mount destination')
        destinations.add(item['Destination'])
    return sorted(records, key=lambda item: json.dumps(item, sort_keys=True, separators=(',', ':')))


def same_inspections(current, expected, phase):
    """Distinguish actual identity changes from full-field mount changes."""
    need(set(current) == set(expected), 'container membership changed ' + phase)
    for name in expected:
        before, after = expected[name], current[name]
        need({k: v for k, v in before.items() if k != 'Mounts'}
             == {k: v for k, v in after.items() if k != 'Mounts'},
             'container identity/birth changed ' + phase + ': ' + name)
        need(canonical_mounts(before['Mounts']) == canonical_mounts(after['Mounts']),
             'container mounts changed ' + phase + ': ' + name)


def inspect_all():
    value = {}
    for name in CONTAINERS:
        run = subprocess.run(['docker', 'inspect', name], capture_output=True, timeout=15, check=True)
        need(len(run.stdout) <= MAX_METADATA, 'inspect metadata limit')
        item = json.loads(run.stdout)[0]
        need(item['State']['Running'] and not item['State']['Paused'], 'container unavailable')
        value[name] = {'Id': item['Id'], 'Image': item['Image'], 'Created': item['Created'],
                       'birth': item['State']['StartedAt'], 'Mounts': canonical_mounts(item.get('Mounts', []))}
    return value


def native(args):
    out = Path(args.native_output)
    result = {'status': 'FAIL', 'mode': args.mode, 'removed': [], 'uncertain': False,
              'scope': 'Two fixed old functional incremental trees only; complete projects preservation, no whole-cache or proof acceptance'}
    try:
        config = load(Path(args.config))
        need(digest(Path(__file__)) == config['controller_sha256'], 'controller changed')
        need(digest(out / 'task-guard.py') == config['guard_sha256'], 'task guard changed')
        need(row(1) is not None and row(1)['start'] == config['pid1_starts'][args.container], 'PID1 birth changed')
        guard = runpy.run_path(str(out / 'task-guard.py'))['dead_threads']
        if args.mode == 'holders':
            proof = load(out / 'inventory.json') if (out / 'inventory.json').exists() else None
            inodes = {(v['device'], v['inode']) for v in proof['entries'].values()} if proof else set()
            result = holders(config['aliases'][args.container], inodes, guard)
        elif args.mode == 'inventory':
            # Both exact scratch roots must be actual directories, never linked
            # namespace prefixes. Full projects membership is then frozen below.
            for raw in SCRATCH_ROOTS:
                root = Path(raw)
                for prefix in reversed((root,) + tuple(root.parents)):
                    directory_id(prefix)
            value = inventory()
            validate_inventory(value)
            save(out / 'inventory.json', value)
            result.update(status='PASS', inventory_sha256=digest(out / 'inventory.json'),
                          candidates=len(value['candidates']), allocated_bytes=value['candidate_allocated_bytes'])
        elif args.mode == 'retire':
            approved = validate_inventory(load(out / 'approved-inventory.json'))
            need(digest(out / 'approved-inventory.json') == config['approved_sha256'], 'approval bytes changed')
            # Host has just made a fresh complete inventory before BOTH
            # namespace censuses. Do not insert another full hash sweep between
            # the dual census and unlink; every candidate is checked below.
            current = validate_inventory(load(out / 'inventory.json'))
            need(current == approved, 'fresh approved inventory changed')
            inodes = {(v['device'], v['inode']) for v in current['entries'].values()}
            audit = holders(config['aliases'][args.container], inodes, guard)
            save(out / 'immediate-holders.json', audit)
            need(audit['complete'], 'live or uncertain holders')
            for root, expected in approved['root_ids'].items():
                need(directory_id(Path(root)) == expected, 'root changed before unlink')
            for name in approved['candidates']:
                wanted = approved['entries'][name]
                path = Path(name)
                # No intervening linked ancestor, replacement or permission change.
                for ancestor in path.parents:
                    if str(ancestor) in approved['entries']:
                        need(directory_id(ancestor) == {k: approved['entries'][str(ancestor)][k]
                             for k in ('device', 'inode', 'mode')}, 'ancestor changed')
                need(metadata(path.lstat()) == {k: wanted[k] for k in metadata(path.lstat())}, 'candidate changed')
                fd = open_regular(path)
                try:
                    need(metadata(os.fstat(fd)) == metadata(path.lstat()), 'opened candidate changed')
                    value = hashlib.sha256()
                    with os.fdopen(fd, 'rb', closefd=False) as handle:
                        magic = handle.read(4)
                        need(admitted_file(Path(wanted['relative']), os.fstat(fd), magic), 'final candidate no longer admitted')
                        value.update(magic)
                        for data in iter(lambda: handle.read(1024 * 1024), b''):
                            value.update(data)
                    need(value.hexdigest() == wanted['sha256'], 'candidate bytes changed')
                    need(metadata(os.fstat(fd)) == metadata(path.lstat()) == {k: wanted[k] for k in metadata(path.lstat())}, 'final named/opened candidate changed')
                    need(row(1) is not None and row(1)['start'] == config['pid1_starts'][args.container], 'PID1 changed before unlink')
                    result['pending_unlink'] = name
                    save(out / 'native-result.json', result)
                    path.unlink()
                    result['removed'].append(name)
                    result['pending_unlink'] = None
                    save(out / 'native-result.json', result)
                finally:
                    os.close(fd)
            after = inventory()
            save(out / 'after-inventory.json', after)
            compare_preserved(approved, after, result['removed'])
            result.update(status='PASS', allocated_file_bytes=sum(approved['entries'][p]['allocated_bytes'] for p in result['removed']))
        else:
            need(False, 'unknown native mode')
    except BaseException as error:
        result.update(status='FAIL', uncertain=True, error_type=type(error).__name__, error=str(error)[:256])
    finally:
        save(out / ('holders-' + args.container + '.json' if args.mode == 'holders' else 'native-result.json'), result)
    print(json.dumps(result))
    return 0 if result.get('status') == 'PASS' or result.get('complete') is True else 1


def host(args):
    output = Path(args.output)
    output.mkdir(mode=0o700, exist_ok=False)
    result = {'status': 'FAIL', 'scope': 'Only two fixed old functional incremental trees; complete projects preservation, no whole-cache postcheck',
              'mutation_state': 'not-attempted', 'inventory_only': args.approved_inventory is None,
              'native_output': args.native_output, 'commands': []}
    try:
        before = inspect_all()
        result['containers_before'] = before
        save(output / 'controller.json', result)
        translated = aliases(before)
        result['aliases'] = translated
        starts = {}
        result['pid1_starts'] = starts
        save(output / 'controller.json', result)
        for name in CONTAINERS:
            birth = subprocess.run(['docker', 'exec', name, 'cat', '/proc/1/stat'], capture_output=True, timeout=15, check=True)
            need(len(birth.stdout) <= 4096, 'PID1 stat bound')
            starts[name] = int(birth.stdout.rsplit(b') ', 1)[1].split()[19])
            save(output / 'controller.json', result)
        setup_after = inspect_all()
        result['containers_setup_after'] = setup_after
        save(output / 'controller.json', result)
        same_inspections(setup_after, before, 'during setup')
        config = dict(translated, guard_sha256=digest(Path(args.guard)),
                      controller_sha256=digest(Path(__file__)), pid1_starts=starts)
        if args.approved_inventory:
            config['approved_sha256'] = digest(Path(args.approved_inventory))
        save(output / 'config.json', config)
        result['containers_before'] = before
        result['aliases'] = translated
        def call(command, cap=60):
            same_inspections(inspect_all(), before, 'before command')
            index = len(result['commands'])
            row = {'command': command, 'cap_seconds': cap}
            result['commands'].append(row)
            save(output / 'controller.json', result)
            with (output / (str(index) + '.stdout')).open('wb') as stdout, (output / (str(index) + '.stderr')).open('wb') as stderr:
                code = subprocess.run(command, stdout=stdout, stderr=stderr, timeout=cap, check=False).returncode
            row['exit_code'] = code
            need(code == 0, 'one cleanup command failed; no retry')
            same_inspections(inspect_all(), before, 'after command')
        for name in CONTAINERS:
            call(['docker', 'exec', name, 'mkdir', '-m', '700', args.native_output])
            for path, member in ((Path(__file__), 'controller.py'), (Path(args.guard), 'task-guard.py'), (output / 'config.json', 'config.json')):
                call(['docker', 'cp', str(path), name + ':' + args.native_output + '/' + member])
        def run(name, mode, cap=60):
            call(['docker', 'exec', name, 'python3', args.native_output + '/controller.py',
                  '--native-output', args.native_output, '--config', args.native_output + '/config.json',
                  '--container', name, '--mode', mode], cap)
        run(CONTAINERS[0], 'inventory', 360)
        call(['docker', 'cp', CONTAINERS[0] + ':' + args.native_output + '/inventory.json', str(output / 'inventory.json')])
        call(['docker', 'cp', str(output / 'inventory.json'), CONTAINERS[1] + ':' + args.native_output + '/inventory.json'])
        for name in CONTAINERS:
            run(name, 'holders')
        if args.approved_inventory:
            # Handoff census runs immediately before acceptance retirement; the
            # acceptance controller repeats its own census under approved IDs.
            call(['docker', 'cp', args.approved_inventory, CONTAINERS[0] + ':' + args.native_output + '/approved-inventory.json'])
            result['mutation_state'] = 'attempted-outcome-unconfirmed'
            save(output / 'controller.json', result)
            run(CONTAINERS[0], 'retire', 480)
            result['mutation_state'] = 'completed-native-pass'
        for name in CONTAINERS:
            run(name, 'holders')
            call(['docker', 'cp', name + ':' + args.native_output + '/.', str(output / name)], 60)
        final = inspect_all()
        result['containers_after'] = final
        same_inspections(final, before, 'at final closure')
        result['status'] = 'PASS'
    except BaseException as error:
        result.update(status='FAIL', uncertain=True, error_type=type(error).__name__, error=str(error)[:256])
        # Never start a replacement command or further mutation after failure.
    finally:
        save(output / 'controller.json', result)
    return 0 if result['status'] == 'PASS' else 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output')
    parser.add_argument('--native-output', required=True)
    parser.add_argument('--guard', help='Exact reviewed scripts/local-combined-gates.py')
    parser.add_argument('--approved-inventory', help='Explicit exact prior inventory; omission is inventory-only')
    parser.add_argument('--mode', choices=('inventory', 'holders', 'retire'))
    parser.add_argument('--container', choices=CONTAINERS)
    parser.add_argument('--config')
    args = parser.parse_args()
    def interrupted(number, _frame):
        raise KeyboardInterrupt('retirement controller signal ' + str(number))
    signal.signal(signal.SIGTERM, interrupted)
    signal.signal(signal.SIGHUP, interrupted)
    need(args.native_output.startswith('/tmp/sail-old-functional-incremental-') and '/' not in args.native_output[len('/tmp/sail-old-functional-incremental-'):], 'new external owned evidence path required')
    if args.mode:
        return native(args)
    need(args.output and args.guard, 'host output and exact shared guard required')
    return host(args)


if __name__ == '__main__':
    raise SystemExit(main())
