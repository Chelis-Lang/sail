#!/usr/bin/env python3
"""Retire one closed Sail /tmp run after verified compact evidence transport.

Local-only, no process signals or retries. Full text/JSON/source/receipt files
are retained; compiler trees and historical binaries are not archived. Selected
products/runtime and shared targets must be outside the explicit scratch root.
"""
import argparse
from contextlib import contextmanager
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import stat
import sys
import tarfile
import tempfile
import time
import types

sys.dont_write_bytecode = True
MAX_ENTRIES = 20000
MAX_SCRATCH_BYTES = 8*1024**3
MAX_EVIDENCE_BYTES = 128*1024**2
MAX_METADATA = 16*1024**2
PHASE_SECONDS = 300
TEXT_SUFFIXES = {'.json', '.jsonl', '.log', '.txt', '.stdout', '.stderr', '.rs', '.py', '.sh', '.toml', '.lock', '.md', '.patch', '.csv', '.smt2', '.out', '.err', '.yaml', '.yml'}
TEXT_NAMES = {'stdout', 'stderr', 'output', 'Cargo.lock'}


def need(condition, message):
    if not condition:
        raise ValueError(message)


def module(path, name):
    spec = importlib.util.spec_from_file_location(name, path)
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


def helpers(directory):
    return (module(Path(directory)/'local-combined-gates.py', 'retirement_gate'),
            module(Path(directory)/'retire-old-functional-two-incremental.py', 'retirement_holders'))


def load(path):
    with Path(path).open('rb') as stream:
        raw = stream.read(MAX_METADATA+1)
    need(len(raw) <= MAX_METADATA, 'metadata limit')
    return json.loads(raw)


def save(path, value):
    raw = (json.dumps(value, separators=(',', ':'))+'\n').encode()
    need(len(raw) <= MAX_METADATA, 'metadata limit')
    temporary = Path(str(path)+'.new')
    with os.fdopen(os.open(temporary, os.O_WRONLY|os.O_CREAT|os.O_EXCL, 0o600), 'wb') as stream:
        stream.write(raw)
        stream.flush()
        os.fsync(stream.fileno())
    temporary.replace(path)
    fd = os.open(Path(path).parent, os.O_RDONLY|os.O_DIRECTORY|os.O_NOFOLLOW)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def sha(path, limit=MAX_EVIDENCE_BYTES, deadline=None):
    h, count = hashlib.sha256(), 0
    deadline = deadline if deadline is not None else time.monotonic()+PHASE_SECONDS
    with Path(path).open('rb') as stream:
        for block in iter(lambda: stream.read(1024**2), b''):
            count += len(block)
            need(count <= limit and time.monotonic() < deadline, 'content byte/time bound')
            h.update(block)
    return h.hexdigest()


def validate_root(root, protected):
    root = Path(root)
    need(root.parent == Path('/tmp') and root.name.startswith('sail-') and root.name != 'sail-' and not root.name.startswith('sail-run-retirement-') and '..' not in root.parts, 'one explicit Sail /tmp run required')
    for raw in protected:
        path = Path(raw)
        need(path.is_absolute() and '..' not in path.parts, 'protected path must be absolute')
        need(path != root and not path.is_relative_to(root) and not root.is_relative_to(path), 'protected path overlaps scratch')
    return root


def durable_output(output):
    """Host evidence must outlive disposable native and host /tmp runs."""
    path = Path(output).resolve()
    need(path.is_absolute() and not any(path.is_relative_to(root.resolve()) for root in
         (Path('/tmp'), Path('/var/tmp'), Path(tempfile.gettempdir())))
         and path != Path('/'), 'host evidence must be outside temporary storage')
    return path


def terminal_closed(terminal):
    """Original PASS/FAIL is preserved; missing or uncertain closure defers cleanup."""
    need(terminal.get('status') in ('PASS', 'FAIL') and not terminal.get('uncertainty')
         and not terminal.get('uncertain'), 'terminal closure uncertain; separate audit required')
    closure = terminal.get('closure')
    if isinstance(closure, dict):
        need(closure.get('complete') is True and closure.get('candidates') == []
             and not closure.get('unavailable') and not closure.get('unreadable'), 'incomplete terminal census')
        need(terminal.get('closure_complete', True) is True, 'terminal closure incomplete')
    else:
        need(closure == 'original source/protected bytes and PID1 equal; known phase groups closed, not an arbitrary orphan-absence proof', 'matching closed gate record required')
        phases = terminal.get('phases')
        need(isinstance(phases, list) and phases, 'no closed phase evidence')
        g, _h = helpers(Path(__file__).parent)
        need(all(g.closed_groups(phase.get('group_closure', {})) for phase in phases), 'phase group closure unavailable')


def relative_name(name):
    path = Path(name)
    need(isinstance(name, str) and name == str(path) and not path.is_absolute()
         and '..' not in path.parts and bool(path.parts), 'relative evidence path escapes scratch')
    return path


def identity(info):
    return {'device': info.st_dev, 'inode': info.st_ino, 'mode': info.st_mode,
            'bytes': info.st_size, 'links': info.st_nlink, 'uid': info.st_uid,
            'gid': info.st_gid, 'mtime_ns': info.st_mtime_ns, 'ctime_ns': info.st_ctime_ns}


def evidence_file(relative):
    """Only compact textual evidence; target trees never become giant archives."""
    return not {'target', 'target-verus', 'target-old', 'incremental', '__pycache__'}.intersection(relative.parts) and (relative.suffix in TEXT_SUFFIXES or relative.name in TEXT_NAMES)


def inventory(root, deadline=None):
    deadline = deadline if deadline is not None else time.monotonic()+PHASE_SECONDS
    info = root.lstat()
    need(stat.S_ISDIR(info.st_mode) and info.st_uid == os.getuid(), 'owned real scratch directory required')
    value = {'root': str(root), 'root_identity': identity(info), 'parent_identity': identity(root.parent.lstat()), 'entries': {}, 'evidence_bytes': 0, 'scratch_bytes': 0}
    pending = [root]
    while pending:
        path = pending.pop()
        relative = path.relative_to(root)
        need(len(value['entries']) < MAX_ENTRIES and time.monotonic() < deadline, 'inventory entry/time bound')
        info = path.lstat()
        row = identity(info)
        if stat.S_ISDIR(info.st_mode):
            row['kind'] = 'directory'
            with os.scandir(path) as children:
                for child in children:
                    need(len(pending)+len(value['entries']) < MAX_ENTRIES, 'pending entry bound')
                    pending.append(Path(child.path))
        elif stat.S_ISLNK(info.st_mode):
            row.update(kind='symlink', target=os.readlink(path))
            need(len(os.fsencode(row['target'])) <= 4096, 'link byte bound')
        elif stat.S_ISREG(info.st_mode):
            row.update(kind='file', retained=evidence_file(relative))
            value['scratch_bytes'] += info.st_size
            need(value['scratch_bytes'] <= MAX_SCRATCH_BYTES, 'scratch byte bound')
            if row['retained']:
                value['evidence_bytes'] += info.st_size
                need(value['evidence_bytes'] <= MAX_EVIDENCE_BYTES, 'full evidence exceeds compact bound; leave run intact')
                fd = os.open(path, os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK)
                try:
                    need(identity(os.fstat(fd)) == row_identity(row), 'evidence opened identity differs')
                    digest = hashlib.sha256()
                    read = 0
                    with os.fdopen(fd, 'rb', closefd=False) as stream:
                        for block in iter(lambda: stream.read(1024**2), b''):
                            read += len(block)
                            need(read <= info.st_size and time.monotonic() < deadline, 'evidence grew or timed out')
                            digest.update(block)
                    need(read == info.st_size and identity(os.fstat(fd)) == identity(path.lstat()) == row_identity(row), 'evidence changed')
                    row['sha256'] = digest.hexdigest()
                finally:
                    os.close(fd)
        else:
            need(False, 'special scratch entry; leave run intact')
        value['entries'][str(relative)] = row
    return value


def row_identity(row):
    return {key: row[key] for key in ('device', 'inode', 'mode', 'bytes', 'links', 'uid', 'gid', 'mtime_ns', 'ctime_ns')}


@contextmanager
def bound_parent(root, plan, relative):
    """Every ancestor is opened without following links and checked to the plan."""
    fd = os.open(root.parent, os.O_RDONLY|os.O_DIRECTORY|os.O_NOFOLLOW)
    try:
        need(all(identity(os.fstat(fd))[key] == plan['parent_identity'][key]
                 for key in ('device', 'inode', 'mode', 'uid', 'gid')), 'scratch parent binding changed')
        if relative != Path('.'):
            for index, component in enumerate((root.name, *relative.parts[:-1])):
                child = os.open(component, os.O_RDONLY|os.O_DIRECTORY|os.O_NOFOLLOW, dir_fd=fd)
                os.close(fd)
                fd = child
                name = '.' if index == 0 else str(Path(*relative.parts[:index]))
                expected = plan['entries'][name]
                need(all(identity(os.fstat(fd))[key] == expected[key]
                         for key in ('device', 'inode', 'mode', 'uid', 'gid')), 'ancestor binding changed')
        yield fd
    finally:
        os.close(fd)


def validate_plan(root, plan):
    need(plan.get('root') == str(root) and isinstance(plan.get('entries'), dict)
         and 0 < len(plan['entries']) <= MAX_ENTRIES, 'inventory root/count differs')
    need(isinstance(plan.get('parent_identity'), dict), 'scratch parent binding missing')
    need(plan['entries'].get('.', {}).get('kind') == 'directory'
         and row_identity(plan['entries']['.']) == plan['root_identity'], 'root binding missing')
    scratch = evidence = 0
    for name, row in plan['entries'].items():
        path = Path('.') if name == '.' else relative_name(name)
        kind = row['kind']
        need(kind in ('file', 'directory', 'symlink') and row['bytes'] >= 0, 'unsupported inventory member')
        need({'file': stat.S_ISREG, 'directory': stat.S_ISDIR, 'symlink': stat.S_ISLNK}[kind](row['mode']), 'inventory type differs')
        if name != '.':
            need(plan['entries'].get(str(path.parent), {}).get('kind') == 'directory', 'inventory parent missing')
        if kind == 'file':
            scratch += row['bytes']
            need(row.get('retained') is evidence_file(path), 'inventory retention differs')
            if row['retained']:
                evidence += row['bytes']
                need(isinstance(row.get('sha256'), str) and len(row['sha256']) == 64, 'evidence hash missing')
    need(scratch == plan['scratch_bytes'] <= MAX_SCRATCH_BYTES
         and evidence == plan['evidence_bytes'] <= MAX_EVIDENCE_BYTES, 'inventory byte accounting differs')


def archive(root, plan, path, deadline=None):
    deadline = deadline if deadline is not None else time.monotonic()+PHASE_SECONDS
    with tarfile.open(path, 'x:gz', format=tarfile.PAX_FORMAT) as output:
        for name, row in plan['entries'].items():
            if not row.get('retained'):
                continue
            need(time.monotonic() < deadline, 'archive time bound')
            source = root/name
            with bound_parent(root, plan, Path(name)) as parent_fd:
                fd = os.open(Path(name).name, os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK, dir_fd=parent_fd)
            try:
                need(identity(os.fstat(fd)) == identity(source.lstat()) == row_identity(row), 'archive source changed')
                item = tarfile.TarInfo(name)
                item.size, item.mode, item.uid, item.gid = row['bytes'], stat.S_IMODE(row['mode']), row['uid'], row['gid']
                with os.fdopen(fd, 'rb', closefd=False) as stream:
                    output.addfile(item, stream)
                need(identity(os.fstat(fd)) == identity(source.lstat()) == row_identity(row), 'archive source changed during read')
            finally:
                os.close(fd)
    path.chmod(0o600)
    with path.open('rb') as stream:
        os.fsync(stream.fileno())


def verify_archive(path, plan, deadline=None):
    """Full member bytes/modes and exact coverage; never normalize raw logs."""
    expected = {name: row for name, row in plan['entries'].items() if row.get('retained')}
    seen = set()
    deadline = deadline if deadline is not None else time.monotonic()+PHASE_SECONDS
    with tarfile.open(path, 'r|gz') as archive_file:
        for item in archive_file:
            name = item.name
            need(name in expected and name not in seen and item.isfile(), 'archive member differs')
            row = expected[name]
            need((item.size, item.mode, item.uid, item.gid) == (row['bytes'], stat.S_IMODE(row['mode']), row['uid'], row['gid']), 'archive metadata differs')
            digest, count = hashlib.sha256(), 0
            with archive_file.extractfile(item) as stream:
                for block in iter(lambda: stream.read(1024**2), b''):
                    count += len(block)
                    need(count <= row['bytes'] and time.monotonic() < deadline, 'archive content bound')
                    digest.update(block)
            need(count == row['bytes'] and digest.hexdigest() == row['sha256'], 'archive bytes differ')
            seen.add(name)
    need(seen == set(expected), 'missing evidence archive member')


def validate_mounts(mounts, root):
    """Refuse actual mounted storage beneath or containing the scratch root."""
    need(len(mounts) <= 1024**2, 'mount inventory limit')
    for line in mounts.splitlines():
        fields = line.split()
        need(len(fields) >= 6, 'mount inventory malformed')
        raw = fields[4]
        for escaped, literal in ((b'\\040', b' '), (b'\\011', b'\t'), (b'\\012', b'\n'), (b'\\134', b'\\')):
            raw = raw.replace(escaped, literal)
        point = Path(os.fsdecode(raw))
        need(point == Path('/') or (point != root and not point.is_relative_to(root)
             and not root.is_relative_to(point)), 'scratch overlaps native mount')


def audit(g, h, root, plan, require_idle):
    with Path('/proc/self/mountinfo').open('rb') as stream:
        mounts = stream.read(1024**2+1)
    validate_mounts(mounts, root)
    result = h.holders([str(root)], {(row['device'], row['inode']) for row in plan['entries'].values()}, g.dead_threads)
    need(result['complete'], 'held scratch or unavailable holder census')
    if require_idle:
        own = h.row(os.getpid())
        need(own is not None, 'observer birth unavailable')
        count, deadline = 0, time.monotonic()+30
        with os.scandir('/proc') as processes:
            for process in processes:
                if not process.name.isdecimal():
                    continue
                count += 1
                need(count <= 8192 and time.monotonic() < deadline, 'idle census bound')
                row = h.row(int(process.name))
                need(row is not None, 'idle process birth unavailable')
                if row['pid'] in (1, own['pid']):
                    continue
                need(row['state'] in ('Z','X') and g.dead_threads(types.SimpleNamespace(row=h.row), row)['complete'], 'container not idle')
        need(h.row(own['pid']) == own, 'observer birth changed')
    return result


def delete_scratch(root, plan, checkpoint, deadline=None):
    """No-follow unlink, original inode binding and explicit partial journal."""
    deadline = deadline if deadline is not None else time.monotonic()+PHASE_SECONDS
    adjusted = {}
    removed = []
    for name, row in sorted(plan['entries'].items(), key=lambda item: len(Path(item[0]).parts), reverse=True):
        need(time.monotonic() < deadline, 'retirement time bound')
        path = root/name
        with bound_parent(root, plan, Path(name)) as parent_fd:
            current = identity(os.stat(path.name,dir_fd=parent_fd,follow_symlinks=False))
            if row['kind'] == 'directory':
                need(all(current[key] == row[key] for key in ('device','inode','mode','uid','gid')), 'directory replaced')
                checkpoint(removed,name)
                os.rmdir(path.name,dir_fd=parent_fd)
            else:
                key = (row['device'], row['inode'])
                expected = adjusted.get(key, row_identity(row))
                need(current == expected, 'scratch member replaced')
                fd = os.open(path.name,os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK,dir_fd=parent_fd) if row['kind'] == 'file' else None
                try:
                    if fd is not None:
                        need(identity(os.fstat(fd)) == expected, 'scratch opened identity differs')
                    checkpoint(removed,name)
                    os.unlink(path.name,dir_fd=parent_fd)
                    if fd is not None:
                        after = identity(os.fstat(fd))
                        need(after['links'] == expected['links']-1 and all(after[k] == expected[k] for k in expected if k not in ('links','ctime_ns')), 'outside inode changed')
                        adjusted[key] = after
                finally:
                    if fd is not None:
                        os.close(fd)
        removed.append(name)
        checkpoint(removed, None)
    return removed


def native(args):
    out = Path(args.native)
    need(out.parent == Path('/tmp') and out.name.startswith('sail-run-retirement-')
         and '..' not in out.parts and args.mode in ('capture', 'holders', 'retire'), 'external native cleanup directory/mode required')
    output_info = out.lstat()
    need(stat.S_ISDIR(output_info.st_mode) and output_info.st_uid == os.getuid()
         and stat.S_IMODE(output_info.st_mode) == 0o700, 'private owned native cleanup directory required')
    g, h = helpers(out)
    config = load(out/'config.json')
    root = validate_root(config['root'], config['protected'])
    result = {'status':'UNCERTAIN', 'mode':args.mode, 'removed':[], 'removed_count':0, 'pending':None,
              'deletion_order':'descending relative-path depth, stable inventory insertion order'}
    deadline = time.monotonic()+PHASE_SECONDS
    try:
        need(args.container in config['pid1_starts'] and config['protected'], 'owned container/protected inventory required')
        need(sha(Path(__file__)) == config['script_sha256'], 'retirement script changed')
        for name,expected in config['support_sha256'].items():
            need(sha(out/name) == expected, 'retirement support changed')
        need(h.row(1)['start'] == config['pid1_starts'][args.container], 'PID1 birth changed')
        if args.mode == 'capture':
            plan = inventory(root, deadline)
            validate_plan(root, plan)
            result['holders'] = audit(g,h,root,plan,True)
            for required in config['required']:
                need(plan['entries'].get(required,{}).get('retained'), 'required full evidence absent')
            save(out/'inventory.json',plan)
            archive(root,plan,out/'evidence.tar.gz',deadline)
            verify_archive(out/'evidence.tar.gz',plan,deadline)
            need(inventory(root,deadline) == plan, 'scratch changed during evidence capture')
            result.update(archive_sha256=sha(out/'evidence.tar.gz',deadline=deadline), inventory_sha256=sha(out/'inventory.json',MAX_METADATA,deadline))
        else:
            plan = load(out/'inventory.json')
            validate_plan(root, plan)
            need(sha(out/'inventory.json',MAX_METADATA) == config['inventory_sha256'], 'approved inventory changed')
            if args.mode == 'retire':
                need(inventory(root,deadline) == plan, 'scratch changed before retirement')
            result['holders'] = audit(g,h,root,plan,args.container == config['container'])
            if args.mode == 'retire':
                need(args.container == config['container'], 'retirement container differs')
                need(sha(out/'evidence.tar.gz',deadline=deadline) == config['archive_sha256'], 'verified compact backing changed')
                def checkpoint(removed,pending):
                    result.update(removed_count=len(removed),last_removed=removed[-1] if removed else None,pending=pending)
                    save(out/'retire-progress.json',{'status':'UNCERTAIN','removed_count':len(removed),
                         'last_removed':result['last_removed'],'pending':pending,'inventory_sha256':config['inventory_sha256']})
                result['removed'] = delete_scratch(root,plan,checkpoint,deadline)
                need(not root.exists() and not root.is_symlink(), 'scratch remains')
                result['historical_binaries'] = 'retired; source/hash/report records retained, not reexecutable'
        need(time.monotonic() < deadline, 'native phase time bound')
        need(h.row(1)['start'] == config['pid1_starts'][args.container], 'PID1 birth changed at closure')
        result['status'] = 'PASS'
    except BaseException as error:
        result.update(error_type=type(error).__name__,error=str(error)[:256])
    finally:
        save(out/(args.mode+'-'+args.container+'.json'),result)
    return 0 if result['status'] == 'PASS' else 1


def retire_run(container, root, output, terminal, protected, required, host_evidence=()):
    """Host orchestration; original terminal status remains an independent field."""
    g,h = helpers(Path(__file__).parent)
    root = validate_root(root,protected)
    output = durable_output(output)
    output.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    output.mkdir(mode=0o700,exist_ok=False)
    record = {'status':'UNCERTAIN','original_status':terminal.get('status'),'root':str(root),'removed':False}
    def call(command,timeout=60):
        return g.bounded_command(command,timeout=timeout)
    before = None
    try:
        terminal_closed(terminal)
        need(not terminal.get('native_root') or terminal['native_root'] == str(root), 'terminal scratch root differs')
        need(protected, 'explicit external protected paths required')
        for name in required:
            relative_name(name)
        need(required, 'explicit complete log/terminal evidence required')
        save(output/'original-terminal.json',terminal)
        record['host_evidence'] = []
        for source in host_evidence:
            source = Path(source)
            destination = output/'host-evidence'/source.name
            destination.parent.mkdir(mode=0o700, exist_ok=True)
            need(not destination.exists() and source.is_file() and not source.is_symlink(), 'host evidence must be unique regular files')
            before_sha = sha(source, MAX_SCRATCH_BYTES)
            shutil.copyfile(source, destination)
            destination.chmod(0o600)
            with destination.open('rb') as stream:
                os.fsync(stream.fileno())
            need(sha(destination, MAX_SCRATCH_BYTES) == sha(source, MAX_SCRATCH_BYTES) == before_sha, 'host evidence transport differs')
            record['host_evidence'].append({'name': source.name, 'bytes': destination.stat().st_size, 'sha256': before_sha})
        before = h.inspect_all()
        record['containers_before'] = before
        need(container in before,'unsupported owned container')
        for view in before.values():
            need(not any(h.below(str(root),[mount['Destination']]) or h.below(mount['Destination'],[str(root)]) for mount in view['Mounts']), 'scratch overlaps mounted storage')
        config = {'container':container,'root':str(root),'protected':list(protected),'required':list(required),'script_sha256':sha(Path(__file__)), 'pid1_starts':{}, 'support_sha256':{name:sha(Path(__file__).with_name(name)) for name in ('local-combined-gates.py','retire-old-functional-two-incremental.py')}}
        for name in before:
            run=call(['docker','exec',name,'cat','/proc/1/stat'])
            config['pid1_starts'][name]=int(run.stdout.rsplit(b') ',1)[1].split()[19])
        setup = call(['docker','exec',container,'mktemp','-d','/tmp/sail-run-retirement-XXXXXXXX']).stdout.decode().strip()
        need(Path(setup).parent == Path('/tmp') and Path(setup).name.startswith('sail-run-retirement-'), 'external cleanup directory differs')
        record['native_output']=setup
        save(output/'config.json',config)
        def same():
            h.same_inspections(h.inspect_all(),before,'run retirement')
        def copy(source,destination):
            same();call(['docker','cp',str(source),str(destination)]);same()
        support=(Path(__file__),Path(__file__).with_name('local-combined-gates.py'),Path(__file__).with_name('retire-old-functional-two-incremental.py'))
        for name in before:
            if name != container:
                same();call(['docker','exec',name,'mkdir','-m','700',setup]);same()
            for source in support:
                copy(source,name+':'+setup+'/'+source.name)
            copy(output/'config.json',name+':'+setup+'/config.json')
        def run(name,mode):
            same()
            execution=g.bounded_command(['docker','exec',name,'python3','-B',setup+'/'+Path(__file__).name,'--native',setup,'--container',name,'--mode',mode],timeout=PHASE_SECONDS+60,check=False)
            same()
            report_file=mode+'-'+name+'.json'
            copy(name+':'+setup+'/'+report_file,output/report_file)
            terminal_result=load(output/report_file)
            need(execution.returncode == 0 and terminal_result['status'] == 'PASS', 'native '+mode+' failed; no retry')
            return execution
        run(container,'capture')
        for member in ('inventory.json','evidence.tar.gz'):
            copy(container+':'+setup+'/'+member,output/member)
            (output/member).chmod(0o600)
        plan=load(output/'inventory.json')
        validate_plan(root,plan)
        verify_archive(output/'evidence.tar.gz',plan)
        capture=load(output/('capture-'+container+'.json'))
        config.update(inventory_sha256=sha(output/'inventory.json',MAX_METADATA),archive_sha256=sha(output/'evidence.tar.gz'))
        need(capture['status'] == 'PASS' and capture['inventory_sha256'] == config['inventory_sha256'] and capture['archive_sha256'] == config['archive_sha256'],'transported evidence differs')
        record['verified_evidence']={'archive_sha256':config['archive_sha256'],'inventory_sha256':config['inventory_sha256'],'full_evidence_bytes':plan['evidence_bytes']}
        save(output/'config.json',config)
        for name in before:
            copy(output/'config.json',name+':'+setup+'/config.json')
            if name != container:
                copy(output/'inventory.json',name+':'+setup+'/inventory.json')
        for name in before:
            run(name,'holders')
        record['retirement_attempted']=True
        save(output/'controller.json',record)
        run(container,'retire')
        retired=load(output/('retire-'+container+'.json'))
        need(retired['status'] == 'PASS' and retired['pending'] is None
             and len(retired['removed']) == len(plan['entries'])
             and set(retired['removed']) == set(plan['entries']), 'retirement incomplete')
        record['containers_after'] = h.inspect_all()
        h.same_inspections(record['containers_after'], before, 'final retirement closure')
        record.update(status='PASS',removed=True,retired_entries=len(retired['removed']),historical_binaries='retired/nonreexecutable; no giant compiler archive')
    except BaseException as error:
        record.update(error_type=type(error).__name__,error=str(error)[:256])
    finally:
        save(output/'controller.json',record)
    return record


if __name__ == '__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--container',required=True)
    parser.add_argument('--root')
    parser.add_argument('--output')
    parser.add_argument('--terminal-record')
    parser.add_argument('--protected',action='append',default=[])
    parser.add_argument('--required',action='append',default=[])
    parser.add_argument('--host-evidence',action='append',default=[],help='already retained source archives/logs copied and reverified before scratch deletion')
    parser.add_argument('--native')
    parser.add_argument('--mode',choices=('capture','holders','retire'))
    args=parser.parse_args()
    if args.native:
        raise SystemExit(native(args))
    need(args.root and args.output and args.terminal_record,'explicit root/output/terminal-record required')
    result=retire_run(args.container,args.root,args.output,load(args.terminal_record),args.protected,args.required,[args.terminal_record,*args.host_evidence])
    raise SystemExit(0 if result['status']=='PASS' else 1)
