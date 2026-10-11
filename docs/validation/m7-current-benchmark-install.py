#!/usr/bin/env python3
"""Install exact sealed product/source bytes in acceptance; no Cargo or proof."""

import copy
import hashlib
import importlib.util
import inspect
import json
import os
from pathlib import Path, PurePosixPath
import re
import stat
import subprocess
import sys
import tarfile
import time

HELPER = Path(__file__).with_name(('sail-' if Path(__file__).name.startswith('sail-') else '') + 'm7-current-benchmark-controller.py')
SPEC = importlib.util.spec_from_file_location('benchmark_controller', HELPER)
M = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(M)
INSTALLER = 'scripts/m7-native-product.py'
MAX_ARCHIVE = 512 * 1024 * 1024
MAX_MEMBERS = 100000


def source_archive(path, expected):
    """Check the producer's Git archive and its safe, finite extraction shape."""
    metadata = Path(path).lstat()
    M.require(stat.S_ISREG(metadata.st_mode) and metadata.st_size <= MAX_ARCHIVE, 'Invalid source archive type/size')
    M.require(M.sha(path) == expected, 'Producer source archive differs')
    names, links, total, inventory = set(), set(), 0, {}
    installer_sha = None
    with tarfile.open(path, 'r:') as archive:
        for count, member in enumerate(archive):
            M.require(count < MAX_MEMBERS, 'Source member bound exceeded')
            name = PurePosixPath(member.name)
            M.require(not name.is_absolute() and '..' not in name.parts and len(member.name.encode()) <= 4096, 'Unsafe source member path')
            M.require(member.name not in names, 'Duplicate source member')
            names.add(member.name)
            M.require(member.isfile() or member.isdir() or member.issym(), 'Unsupported source member type')
            total += member.size
            M.require(total <= MAX_ARCHIVE, 'Extracted source byte bound exceeded')
            if member.issym():
                target = PurePosixPath(member.linkname)
                M.require(not target.is_absolute(), 'Absolute source symlink')
                depth = len(name.parent.parts)
                for component in target.parts:
                    depth += -1 if component == '..' else 0 if component == '.' else 1
                    M.require(depth >= 0, 'Source symlink escapes extraction root')
                links.add(name)
                inventory[str(name)] = {'kind': 'symlink', 'target': member.linkname, 'mode': member.mode & 0o777}
            elif member.isdir():
                inventory[str(name)] = {'kind': 'directory', 'mode': member.mode & 0o777}
            else:
                digest = hashlib.sha256()
                with archive.extractfile(member) as stream:
                    for block in iter(lambda: stream.read(1024 * 1024), b''):
                        digest.update(block)
                inventory[str(name)] = {'kind': 'file', 'bytes': member.size, 'sha256': digest.hexdigest(), 'mode': member.mode & 0o777}
            if member.name == INSTALLER:
                M.require(member.isfile() and member.size <= 1024 * 1024, 'Invalid product installer')
                with archive.extractfile(member) as stream:
                    installer_sha = hashlib.sha256(stream.read(1024 * 1024 + 1)).hexdigest()
        for name in names:
            M.require(not any(parent in links for parent in PurePosixPath(name).parents), 'Source member nested beneath symlink')
    M.require(installer_sha is not None, 'Product installer absent')
    return {'sha256': expected, 'bytes': metadata.st_size, 'members': len(names), 'expanded_regular_bytes': total, 'installer_sha256': installer_sha, 'inventory': inventory}


def verify_source(root, inventory):
    """Check every extracted tracked byte/link and reject extra source entries."""
    root = Path(root)
    seen = set()
    for directory, directories, files in os.walk(root, followlinks=False):
        for name in directories + files:
            path = Path(directory) / name
            key = str(path.relative_to(root))
            row = inventory.get(key)
            require(row is not None, 'Unexpected extracted source entry')
            metadata = path.lstat()
            require(stat.S_IMODE(metadata.st_mode) == row['mode'], 'Source mode differs')
            if row['kind'] == 'symlink':
                require(stat.S_ISLNK(metadata.st_mode) and os.readlink(path) == row['target'], 'Source symlink differs')
            elif row['kind'] == 'directory':
                require(stat.S_ISDIR(metadata.st_mode), 'Source directory differs')
            else:
                require(stat.S_ISREG(metadata.st_mode) and metadata.st_size == row['bytes'] and sha(path) == row['sha256'], 'Source bytes differ')
            seen.add(key)
    require(seen == set(inventory), 'Source inventory incomplete')
    return {'status': 'PASS', 'entries': len(seen)}


# The same pure verification function executes remotely; these aliases are only
# used by its local controls. No shell expansion or divergent verifier path.
require = M.require
sha = M.sha


def product_seals(container, directory, artifacts):
    # Safe argv transport, no shell interpolation or environment disclosure.
    program = '''import hashlib,json,os,stat,sys
from pathlib import Path
p=Path(sys.argv[1]);expected=json.loads(sys.argv[2]);rows={}
if p.is_symlink() or stat.S_IMODE(p.stat().st_mode)!=0o700:raise ValueError('product directory not private')
for name,want in expected.items():
 f=p/name;s=f.lstat()
 if not stat.S_ISREG(s.st_mode) or s.st_size!=want['bytes'] or stat.S_IMODE(s.st_mode)!=0o700:raise ValueError('product type/size/mode differs')
 h=hashlib.sha256()
 with f.open('rb') as stream:
  for block in iter(lambda:stream.read(1024*1024),b''):h.update(block)
 if h.hexdigest()!=want['sha256']:raise ValueError('product bytes differ')
 rows[name]={'bytes':s.st_size,'sha256':h.hexdigest(),'uid':s.st_uid,'gid':s.st_gid,'mode':'0700'}
print(json.dumps({'files':rows,'fsuid':int(Path('/proc/self/status').read_text().split('Uid:',1)[1].splitlines()[0].split()[3])}))
'''
    return json.loads(M.docker(container, 'python3', '-c', program, directory, json.dumps(artifacts)))


def execution_record(producer, producer_sha, installation, birth, command, duration):
    M.require(installation['authority'] == 'installation-only' and installation['source_modified'] is False, 'Installer authority differs')
    for name, wanted in producer['artifacts'].items():
        actual = installation['files'][name]
        M.require(all(actual[key] == wanted[key] for key in ('bytes', 'sha256')), 'Installed bytes differ')
    result = copy.deepcopy(producer)
    result.update(container='sail-m7-acceptance', producer_container=producer['container'], producer_grant_sha256=producer_sha, producer_source_path=producer['source_path'], installation=installation, execution_fsuid=installation['credentials']['fsuid'], execution_container_birth=birth, scope='Byte-identical source and fresh service-owned product installation in acceptance only; no benchmark, proof, prewarm or Cargo', engine_build='/root/.cache/sail/verus-src-build/426d8b01e7ffb910a36c15e1f870ff985bb61eef-c77e1a1879302e06')
    # Preserve the true producer build; replace only the installation phase with
    # this actual destination installation. Original producer record is intact.
    result['phases'] = [copy.deepcopy(row) for row in producer['phases'] if row['phase'] != 'owned-install']
    result['phases'].append({'phase': 'owned-install', 'exit_code': 0, 'command': command, 'elapsed_seconds': duration, 'container': 'sail-m7-acceptance'})
    return M.validate_grant(result, producer['source_commit'][:7])


def install(revision):
    M.require(re.fullmatch('[0-9a-f]{7}', revision), 'Use the product short revision')
    directory = Path('/tmp/sail-m7-core-product-' + revision)
    producer_path = directory / 'product-grant.json'
    producer_sha = M.sha(producer_path)
    producer = M.validate_grant(M.read_json(producer_path), revision)
    M.require(producer['container'] == 'sail-m7-handoff-validation', 'Unexpected product producer')
    execution_path = directory / 'product-grant-acceptance.json'
    M.require(not execution_path.exists(), 'Execution record already exists')
    out = directory / 'acceptance-install'
    out.mkdir(mode=0o700, exist_ok=False)
    native = '/tmp/sail-m7-benchmark-install-' + revision
    destination = 'sail-m7-acceptance'
    record = {'version': 1, 'status': 'running', 'scope': 'Byte installation only; no Cargo, proof, prewarm, cache or receipt copies', 'producer_grant_sha256': producer_sha, 'controller_sha256': M.sha(__file__), 'helper_sha256': M.sha(HELPER), 'source_commit': producer['source_commit'], 'commands': [], 'started_unix_ns': time.time_ns()}
    started = time.monotonic()
    M.checkpoint(out / 'controller.json', record)

    def command(name, argv, stdin=None):
        start = time.monotonic()
        row = {'name': name, 'command': argv, 'status': 'running'}
        record['commands'].append(row)
        M.checkpoint(out / 'controller.json', record)
        try:
            with (out / (name + '.stdout')).open('wb') as stdout, (out / (name + '.stderr')).open('wb') as stderr:
                result = subprocess.run(argv, stdin=stdin, stdout=stdout, stderr=stderr, timeout=120)
            row.update(status='PASS' if result.returncode == 0 else 'FAIL', exit_code=result.returncode)
            M.require(result.returncode == 0, 'Installation command failed: ' + name)
        except BaseException as error:
            row.update(status='FAIL', error_type=type(error).__name__, error=str(error))
            raise
        finally:
            row['elapsed_seconds'] = time.monotonic() - start
            M.checkpoint(out / 'controller.json', record)
        return row

    try:
        record['source_archive'] = source_archive(directory / 'source.tar', producer['source_archive_sha256'])
        inventory = record['source_archive'].pop('inventory')
        source_inventory = out / 'source-inventory.json'
        source_inventory.write_text(json.dumps(inventory) + '\n')
        M.require(source_inventory.stat().st_size <= 1024 * 1024, 'Source inventory metadata bound exceeded')
        record['source_inventory_sha256'] = M.sha(source_inventory)
        record['producer_birth_before'] = M.container_identity(producer['container'])
        record['acceptance_birth_before'] = M.container_identity(destination)
        record['producer_before'] = product_seals(producer['container'], producer['product_dir'], producer['artifacts'])
        # Refuse ALL existing destination paths before any extraction/copy.
        preflight = 'import os,sys\nif any(os.path.lexists(p) for p in sys.argv[1:]):raise ValueError("existing destination")\n'
        command('destination-check', ['docker', 'exec', destination, 'python3', '-c', preflight, native, producer['source_path'], producer['product_dir']])
        command('create-staging', ['docker', 'exec', destination, 'mkdir', '-m', '700', native, producer['source_path']])
        with (directory / 'source.tar').open('rb') as stream:
            command('source-extraction', ['docker', 'exec', '-i', destination, 'tar', 'xf', '-', '-C', producer['source_path']], stdin=stream)
        command('source-inventory-copy', ['docker', 'cp', str(source_inventory), destination + ':' + native + '/source-inventory.json'])
        M.require(M.docker(destination, 'sha256sum', native + '/source-inventory.json').split()[0] == record['source_inventory_sha256'], 'Transferred source inventory differs')
        verify_program = 'import os,json,stat,hashlib,sys\nfrom pathlib import Path\n' + inspect.getsource(M.require) + '\n' + inspect.getsource(M.sha) + '\n' + inspect.getsource(verify_source) + '\nprint(json.dumps(verify_source(sys.argv[1],json.loads(Path(sys.argv[2]).read_text()))))\n'
        record['source_verifier_sha256'] = hashlib.sha256(verify_program.encode()).hexdigest()
        command('source-verification', ['docker', 'exec', destination, 'python3', '-c', verify_program, producer['source_path'], native + '/source-inventory.json'])
        installer = producer['source_path'] + '/' + INSTALLER
        M.require(M.docker(destination, 'sha256sum', installer).split()[0] == record['source_archive']['installer_sha256'], 'Extracted installer differs')
        for name, wanted in producer['artifacts'].items():
            target = out / name
            command('receive-' + name, ['docker', 'cp', producer['container'] + ':' + producer['product_dir'] + '/' + name, str(target)])
            M.require(target.stat().st_size == wanted['bytes'] and M.sha(target) == wanted['sha256'], 'Transferred product differs')
            command('stage-' + name, ['docker', 'cp', str(target), destination + ':' + native + '/' + name])
        expected = out / 'expected.json'
        expected.write_text(json.dumps(producer['artifacts']) + '\n')
        command('expected-copy', ['docker', 'cp', str(expected), destination + ':' + native + '/expected.json'])
        invocation = ['docker', 'exec', destination, 'python3', installer, 'install', '--source', native, '--destination', producer['product_dir'], '--expected', native + '/expected.json']
        phase = command('owned-install', invocation)
        installation = M.read_json(out / 'owned-install.stdout')
        record['acceptance_after'] = product_seals(destination, producer['product_dir'], producer['artifacts'])
        M.require(record['acceptance_after']['files'] == installation['files'] and record['acceptance_after']['fsuid'] == installation['credentials']['fsuid'], 'Actual destination installation differs')
        record['producer_after'] = product_seals(producer['container'], producer['product_dir'], producer['artifacts'])
        record['producer_birth_after'] = M.container_identity(producer['container'])
        record['acceptance_birth_after'] = M.container_identity(destination)
        command('source-verification-after', ['docker', 'exec', destination, 'python3', '-c', verify_program, producer['source_path'], native + '/source-inventory.json'])
        M.require(M.docker(destination, 'sha256sum', native + '/source-inventory.json').split()[0] == record['source_inventory_sha256'], 'Post-install source inventory differs')
        M.require(record['producer_before'] == record['producer_after'], 'Producer binaries changed')
        M.require(record['producer_birth_before'] == record['producer_birth_after'], 'Producer container birth changed')
        M.require(record['acceptance_birth_before'] == record['acceptance_birth_after'], 'Acceptance container birth changed')
        M.require(M.sha(producer_path) == producer_sha, 'Producer record changed')
        M.require(M.sha(directory / 'source.tar') == producer['source_archive_sha256'], 'Producer archive changed')
        execution = execution_record(producer, producer_sha, installation, record['acceptance_birth_after'], invocation, phase['elapsed_seconds'])
        M.checkpoint(execution_path, execution)
        record.update(status='PASS', execution_grant_sha256=M.sha(execution_path), source_archive_after_sha256=M.sha(directory / 'source.tar'))
    except BaseException as error:
        record.update(status='FAIL', error_type=type(error).__name__, error=str(error), execution_grant_published=False)
    finally:
        if record['status'] != 'PASS':
            # Failed installation is retained too; a missing/partial destination
            # never creates an execution record or becomes a retry instruction.
            for name, operation in (
                ('producer_grant_after_sha256', lambda: M.sha(producer_path)),
                ('source_archive_after_sha256', lambda: M.sha(directory / 'source.tar')),
                ('producer_after', lambda: product_seals(producer['container'], producer['product_dir'], producer['artifacts'])),
                ('producer_birth_after', lambda: M.container_identity(producer['container'])),
                ('acceptance_birth_after', lambda: M.container_identity(destination)),
            ):
                try:
                    record[name] = operation()
                except BaseException as error:
                    record[name + '_error'] = {'type': type(error).__name__, 'message': str(error)}
        record['elapsed_seconds'] = time.monotonic() - started
        M.checkpoint(out / 'controller.json', record)
        print(json.dumps({'status': record['status'], 'record': str(out / 'controller.json')}), flush=True)
    return 0 if record['status'] == 'PASS' else 1


if __name__ == '__main__':
    if len(sys.argv) != 2:
        raise SystemExit('usage: m7-current-benchmark-install.py SHORT_REV')
    raise SystemExit(install(sys.argv[1]))
