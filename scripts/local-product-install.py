#!/usr/bin/env python3
"""Build/install an exact COMPLETE combined-gate product; installation-only evidence."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import signal
import stat
import sys
import tarfile
import time

sys.dont_write_bytecode = True
BUILD_LIMIT = 600
INSTALL_LIMIT = 60
BUILD_COMMAND = ['cargo', 'build', '--release', '--locked', '-p', 'sail-cli', '-p', 'sail-shim']


def module(path, name):
    spec = importlib.util.spec_from_file_location(name, path)
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


def gate_module():
    return module(Path(__file__).with_name('local-combined-gates.py'), 'combined_gates')


def prerequisite(g, controller, record, config):
    g.validate_terminal(record, config['revision'], config['archive_sha256'])
    if record['status'] != 'PASS' or controller['status'] != 'PASS' or controller.get('uncertainty') or controller.get('transport_exit_code') != 0:
        raise ValueError('exact COMPLETE combined gate and solver PASS required')
    if controller['container_before'] != controller['container_after'] or controller['revision'] != config['revision'] or controller['archive_sha256'] != config['archive_sha256']:
        raise ValueError('combined source/container identity differs')
    if record.get('jobs') != 2 or record.get('nextest_independent_tests') != 1 or any(not g.closed_groups(p.get('group_closure')) for p in record['phases']):
        raise ValueError('complete recorded phase closure/concurrency required')
    if not re.fullmatch('[0-9a-f]{40}', config['revision']):
        raise ValueError('full Git commit required')
    if config['source'] != '/tmp/sail-m7-core-product-source-' + config['revision'][:7]:
        raise ValueError('source does not match common product grant path')


def regular_hash(r, path, limit, deadline):
    metadata = path.lstat()
    if not stat.S_ISREG(metadata.st_mode):
        raise ValueError('retained evidence must be an ordinary file')
    return r.sha(path, limit=limit, deadline=deadline)


def restage_gate(g, gate_output, backing, destination):
    """Restore verified retired records, never their source tree or qualification."""
    r = module(Path(__file__).with_name('retire-completed-run.py'), 'gate_retirement')
    deadline = time.monotonic() + r.PHASE_SECONDS
    controller = g.load(gate_output / 'controller.json')
    config = g.load(gate_output / 'config.json')
    result = g.load(gate_output / 'result.json')
    prerequisite(g, controller, result, config)
    retirement = r.load(backing / 'controller.json')
    if (retirement != controller.get('retirement') or retirement.get('status') != 'PASS'
            or retirement.get('original_status') != 'PASS' or retirement.get('removed') is not True
            or retirement.get('root') != controller['native_root']
            or not retirement.get('containers_before')
            or retirement['containers_before'] != retirement.get('containers_after')):
        raise ValueError('exact complete successful gate retirement required')
    if r.load(backing / 'original-terminal.json') != dict(result, native_root=controller['native_root']):
        raise ValueError('retired original terminal differs')
    sealed = retirement['verified_evidence']
    archive, inventory = backing / 'evidence.tar.gz', backing / 'inventory.json'
    if (regular_hash(r, archive, r.MAX_EVIDENCE_BYTES, deadline) != sealed['archive_sha256']
            or regular_hash(r, inventory, r.MAX_METADATA, deadline) != sealed['inventory_sha256']):
        raise ValueError('retained archive/inventory differs')
    plan = r.load(inventory)
    r.validate_plan(Path(controller['native_root']), plan)
    if plan['evidence_bytes'] != sealed['full_evidence_bytes']:
        raise ValueError('retained evidence accounting differs')
    r.verify_archive(archive, plan, deadline)
    required = {'source-inventory.json', 'protected.json', 'config.json', 'result.json'}
    required.update(phase['name'] + '.log' for phase in result['phases'])
    members = {name for name, row in plan['entries'].items() if row.get('retained')}
    if not required <= members:
        raise ValueError('incomplete retained gate evidence')
    host_rows = retirement['host_evidence']
    if len({row['name'] for row in host_rows}) != len(host_rows):
        raise ValueError('duplicate retained host evidence')
    for row in host_rows:
        name = r.relative_name(row['name'])
        path = backing / 'host-evidence' / name
        if path.lstat().st_size != row['bytes'] or regular_hash(r, path, r.MAX_SCRATCH_BYTES, deadline) != row['sha256']:
            raise ValueError('retained host evidence differs')
    rows = {row['name']: row for row in host_rows}
    for name in ('source.tar', 'source-inventory.json', 'protected.json', 'result.json'):
        if name not in rows or regular_hash(r, gate_output / name, r.MAX_SCRATCH_BYTES, deadline) != rows[name]['sha256']:
            raise ValueError('original/retained host evidence differs')
    source_archive = backing / 'host-evidence/source.tar'
    if rows['source.tar']['sha256'] != config['archive_sha256']:
        raise ValueError('qualified source archive differs')
    # Only after all backing checks: create a NEW directory, never restore source.
    destination.mkdir(mode=0o700, exist_ok=False)
    extracted = set()
    with tarfile.open(archive, 'r|gz') as packed:
        for item in packed:
            if (item.name not in members or item.name in extracted or not item.isfile()
                    or item.size != plan['entries'][item.name]['bytes']
                    or item.mode != stat.S_IMODE(plan['entries'][item.name]['mode'])):
                raise ValueError('restaging archive member changed')
            path = destination / r.relative_name(item.name)
            path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
            count = 0
            with packed.extractfile(item) as incoming, path.open('xb') as outgoing:
                while block := incoming.read(1024**2):
                    count += len(block)
                    if count > item.size or time.monotonic() >= deadline:
                        raise ValueError('evidence restaging time bound')
                    outgoing.write(block)
            path.chmod(item.mode)
            extracted.add(item.name)
    if extracted != members:
        raise ValueError('restaging archive membership changed')
    # Recheck the extracted bytes against the original inventory, including modes.
    files = {}
    for name in members:
        row, path = plan['entries'][name], destination / name
        if (path.lstat().st_size != row['bytes'] or stat.S_IMODE(path.lstat().st_mode) != stat.S_IMODE(row['mode'])
                or regular_hash(r, path, r.MAX_EVIDENCE_BYTES, deadline) != row['sha256']):
            raise ValueError('restaged member differs')
        files[name] = {'bytes': row['bytes'], 'mode': stat.S_IMODE(row['mode']), 'sha256': row['sha256']}
    for name in ('config.json', 'result.json', 'source-inventory.json', 'protected.json'):
        if g.load(destination / name) != g.load(gate_output / name):
            raise ValueError('restaged/original gate record differs')
    for phase in result['phases']:
        if files[phase['name'] + '.log']['sha256'] != phase['log_sha256']:
            raise ValueError('original gate raw log differs')
    if g.inventory(source_archive) != g.load(destination / 'source-inventory.json'):
        raise ValueError('qualified source membership differs')
    for name, original in (('source.tar', source_archive), ('qualified-controller.json', gate_output / 'controller.json')):
        shutil.copyfile(original, destination / name)
        (destination / name).chmod(0o600)
        digest = regular_hash(r, destination / name, r.MAX_SCRATCH_BYTES, deadline)
        if digest != regular_hash(r, original, r.MAX_SCRATCH_BYTES, deadline):
            raise ValueError('restaged original copy differs')
        if name == 'source.tar' and digest != config['archive_sha256']:
            raise ValueError('qualified source changed during restoration')
        files[name] = {'bytes': (destination / name).stat().st_size, 'mode': 0o600, 'sha256': digest}
    provenance = {'authority': 'restored-qualified-evidence', 're_execution': False,
                  'original_native_root': controller['native_root'], 'source': config['source'],
                  'retirement_controller_sha256': regular_hash(r, backing / 'controller.json', r.MAX_METADATA, deadline),
                  'retained_archive_sha256': sealed['archive_sha256'],
                  'retained_inventory_sha256': sealed['inventory_sha256'], 'files': files}
    g.save(destination / 'restaging.json', provenance)
    # Refuse a backing replacement during restoration as well as corrupted members.
    if (r.load(backing / 'controller.json') != retirement
            or regular_hash(r, archive, r.MAX_EVIDENCE_BYTES, deadline) != sealed['archive_sha256']
            or regular_hash(r, inventory, r.MAX_METADATA, deadline) != sealed['inventory_sha256']):
        raise ValueError('retained backing changed during restoration')
    return provenance


def verify_restaging(g, gate_root, config):
    directory = gate_root.lstat()
    if (not stat.S_ISDIR(directory.st_mode) or stat.S_IMODE(directory.st_mode) != 0o700
            or directory.st_uid != os.geteuid()):
        raise ValueError('private owned restaging directory required')
    provenance = g.load(gate_root / 'restaging.json')
    if (provenance != config['gate_restaging'] or provenance['authority'] != 'restored-qualified-evidence'
            or provenance['re_execution'] is not False or provenance['source'] != config['gate_config']['source']
            or provenance['original_native_root'] != config['gate_controller']['native_root']
            or g.load(gate_root / 'config.json') != config['gate_config']
            or g.load(gate_root / 'qualified-controller.json') != config['gate_controller']):
        raise ValueError('restaged gate provenance differs')
    if set(config['gate_inputs']) != set(provenance['files']) | {'restaging.json'}:
        raise ValueError('restaged gate inventory differs')
    if g.sha(gate_root / 'restaging.json') != config['gate_inputs']['restaging.json']:
        raise ValueError('restaging record seal differs')
    for name, row in provenance['files'].items():
        path = gate_root / name
        metadata = path.lstat()
        if (Path(name).is_absolute() or '..' in Path(name).parts or not stat.S_ISREG(metadata.st_mode)
                or metadata.st_size != row['bytes'] or stat.S_IMODE(metadata.st_mode) != row['mode']
                or g.sha(path) != row['sha256'] or config['gate_inputs'][name] != row['sha256']):
            raise ValueError('restaged gate bytes/mode/type differs')


def engine_seal(g, source, expected):
    root = Path(expected['engine_build'])
    if not root.is_absolute() or root.is_symlink() or '..' in root.parts:
        raise ValueError('exact selected engine path required')
    pin = g.load(source / 'engine/verus/artifact-linux.json')
    manifest_path = root / 'sail-build.json'
    if not stat.S_ISREG(manifest_path.lstat().st_mode) or g.sha(manifest_path) != expected['manifest_sha256']:
        raise ValueError('engine manifest differs')
    manifest = g.load(manifest_path)
    files = expected['files']
    if root.name != pin['variant'] or len(files) != 13 or set(files) != set(manifest['artifacts']):
        raise ValueError('selected variant/complete runtime inventory differs')
    if manifest['features'] != ['sail-interactive:' + pin['patch_sha256'], 'host:x86_64-unknown-linux-gnu', 'source-lock:' + pin['source_lock_sha256']]:
        raise ValueError('engine source features differ')
    rows = []
    for name, row in files.items():
        path = Path(name)
        if path.is_absolute() or '..' in path.parts or manifest['artifacts'][name] != row['sha256']:
            raise ValueError('engine artifact binding differs')
        rows.append(dict(row, path=str(root / name)))
    g.verify_protected({'files': rows})
    runtime = root / 'verus/source/target-verus/release'
    if {str(p.relative_to(root)) for p in runtime.iterdir() if p.is_file() or p.is_symlink()} != set(files):
        raise ValueError('engine direct artifact membership differs')
    return expected


def built_artifacts(g, target):
    rows = {}
    for name in ('sail', 'sail-z3'):
        path = Path(target) / 'release' / name
        metadata = path.lstat()
        if not stat.S_ISREG(metadata.st_mode) or not 0 < metadata.st_size <= 128 * 1024 * 1024:
            raise ValueError('built binary bound/type differs')
        with path.open('rb') as stream:
            if stream.read(4) != b'\x7fELF':
                raise ValueError('expected actual Linux ELF')
        rows[name] = {'bytes': metadata.st_size, 'sha256': g.sha(path)}
    return rows


def product_sequence(run, closure, install_command):
    for name, command, limit in (('release-build', BUILD_COMMAND, BUILD_LIMIT),
                                 ('owned-install', install_command, INSTALL_LIMIT)):
        closure.require()
        if run(name, command, limit) != 0:
            return False
    closure.require()
    return True


def make_grant(g, config, record, artifacts, installation):
    if record['status'] != 'PASS' or record.get('uncertainty') or record.get('closure') != 'closed-recorded-scope':
        raise ValueError('closed product phases required')
    phases = record['phases']
    if [p['name'] for p in phases] != ['release-build', 'owned-install'] or any(p.get('exit_code') != 0 for p in phases):
        raise ValueError('both ordinary product commands must pass')
    if phases[0]['command'] != BUILD_COMMAND or phases[0]['limit_seconds'] != BUILD_LIMIT or phases[1]['limit_seconds'] != INSTALL_LIMIT:
        raise ValueError('ordinary product command/caps differ')
    if installation.get('authority') != 'installation-only' or installation.get('source_modified') is not False or installation.get('directory_mode') != '0700' or set(installation['files']) != set(artifacts):
        raise ValueError('complete owned installation required')
    for name, expected in artifacts.items():
        actual = installation['files'][name]
        if any(actual[k] != expected[k] for k in ('bytes', 'sha256')) or actual['mode'] != '0700' or actual['uid'] != installation['credentials']['fsuid']:
            raise ValueError('installed content/owner differs')
    return {'version': 1, 'source_commit': config['gate_config']['revision'],
            'source_archive_sha256': config['gate_config']['archive_sha256'],
            'source_path': config['gate_config']['source'], 'product_dir': config['product_dir'],
            'container': config['container'], 'execution': config['execution'],
            'execution_container_birth': config['execution_container_birth'], 'engine_build': config['engine']['engine_build'],
            'selected_engine': config['engine'], 'combined_gate_result_sha256': config['gate_inputs']['result.json'],
            'phases': [dict(p, phase=p['name']) for p in phases], 'artifacts': artifacts,
            'installation': installation, 'installation_only': True, 'source_modified': False}


def native(root):
    g = gate_module()
    config = g.load(root / 'product-config.json')
    gate_root, source = Path(config['gate_root']), Path(config['gate_config']['source'])
    # Import the archived lifecycle helper, and bind it to the running file.
    if g.sha(Path(g.__file__)) != config['helper_sha256']:
        raise ValueError('combined lifecycle helper differs')
    verify_restaging(g, gate_root, config)
    who = g.support_module(gate_root)
    owner = who.row(os.getpid())
    if owner is None:
        raise ValueError('product controller birth unavailable')
    g.save(root / 'owner.json', owner)
    g.save(root / 'config.json', dict(config['gate_config']))  # common cancellation primitive
    record = {'status': 'RUNNING', 'phases': [], 'scope': 'ordinary release build and owned installation only', 'jobs': 2}
    closure = g.Closure()
    def check():
        verify_restaging(g, gate_root, config)
        for name, digest in config['gate_inputs'].items():
            if g.sha(gate_root / name) != digest:
                raise ValueError('combined gate evidence changed')
        controller = config['gate_controller']
        gate_record = g.load(gate_root / 'result.json')
        prerequisite(g, controller, gate_record, config['gate_config'])
        for phase in gate_record['phases']:
            if g.sha(gate_root / (phase['name'] + '.log')) != phase['log_sha256']:
                raise ValueError('combined raw phase log changed')
        g.verify_source(source, g.load(gate_root / 'source-inventory.json'))
        if os.readlink(source / 'target') != config['gate_config']['target']:
            raise ValueError('source target differs')
        first = who.row(1)
        if first is None or first['start'] != config['gate_config']['pid1_birth']:
            raise ValueError('container PID1 birth differs')
        g.verify_protected(g.load(gate_root / 'protected.json'))
        engine_seal(g, source, config['engine'])
    runner = g.OwnedPhaseRunner(root, source, config['gate_config']['target'], who, closure, record, check)
    for number in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
        signal.signal(number, runner.interrupt)
    artifacts, installation = None, None
    try:
        check()
        code = runner.run('release-build', BUILD_COMMAND, BUILD_LIMIT)
        if code != 0:
            raise ValueError('ordinary release build failed')
        artifacts = built_artifacts(g, config['gate_config']['target'])
        g.save(root / 'expected-binaries.json', artifacts)
        command = ['python3', '-B', str(source / 'scripts/m7-native-product.py'), 'install',
                   '--source', str(Path(config['gate_config']['target']) / 'release'),
                   '--destination', config['product_dir'], '--expected', str(root / 'expected-binaries.json')]
        if runner.run('owned-install', command, INSTALL_LIMIT) != 0:
            raise ValueError('owned installation failed')
        installation = g.load(root / 'owned-install.log')
        record['status'] = 'PASS'
    except BaseException as error:
        record.update(status='FAIL', error_type=type(error).__name__, error=str(error))
    finally:
        record['uncertainty'] = closure.reasons
        g.save(root / 'primary.json', record)
        try:
            runner.require_idle()
            check()
            if artifacts is not None and built_artifacts(g, config['gate_config']['target']) != artifacts:
                raise ValueError('built binaries changed before closure')
            record['closure'] = 'closed-recorded-scope'
            if record['status'] == 'PASS':
                grant = make_grant(g, config, record, artifacts, installation)
                installed = {'files': [dict(row, path=str(Path(config['product_dir']) / name), mode=int(row['mode'], 8)) for name, row in installation['files'].items()]}
                g.verify_protected(installed)
                directory = Path(config['product_dir']).lstat()
                if not stat.S_ISDIR(directory.st_mode) or stat.S_IMODE(directory.st_mode) != 0o700 or directory.st_uid != installation['credentials']['fsuid']:
                    raise ValueError('installed directory owner/mode differs')
                g.save(root / 'product-grant.json', grant)
                record['product_grant_sha256'] = g.sha(root / 'product-grant.json')
        except BaseException as error:
            record.update(status='FAIL', closure='unavailable', closure_error=str(error))
        g.save(root / 'result.json', record)
    return 0 if record['status'] == 'PASS' else 1


def host(args):
    g = gate_module()
    args.output.mkdir(mode=0o700, exist_ok=False)
    closure = g.Closure()
    report = {'status': 'RUNNING', 'scope': 'ordinary build/owned-install only'}
    before, remote = None, None
    def interrupted(number, _frame):
        closure.uncertain('host interruption')
        for value in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
            signal.signal(value, signal.SIG_IGN)
        raise InterruptedError('product host signal ' + str(number))
    for number in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
        signal.signal(number, interrupted)
    def call(command, timeout=30, **kwargs):
        return g.bounded_command(command, timeout=timeout, closure=closure, **kwargs)
    try:
        if not args.execution or len(args.execution.encode()) > 512 or any(c in args.execution for c in '\r\n'):
            raise ValueError('bounded explicit execution environment label required')
        prior = g.load(args.gate_output / 'controller.json')
        gate_record = g.load(args.gate_output / 'result.json')
        gate_config = g.load(args.gate_output / 'config.json')
        prerequisite(g, prior, gate_record, gate_config)
        before = g.identity(args.container, closure)
        if before != prior['container_after']:
            raise ValueError('current container differs from complete combined gate')
        if not args.native_root.startswith('/tmp/sail-local-product-') or '..' in Path(args.native_root).parts:
            raise ValueError('new private product evidence root required')
        if prior['native_root'] == args.native_root:
            raise ValueError('product output must not modify combined evidence')
        archive = args.gate_output / 'source.tar'
        if g.sha(archive) != gate_config['archive_sha256']:
            raise ValueError('complete combined source archive changed')
        archived = g.inventory(archive)
        for name, actual in (('scripts/local-product-install.py', Path(__file__)), ('scripts/local-combined-gates.py', Path(g.__file__)),
                             ('scripts/retire-completed-run.py', Path(__file__).with_name('retire-completed-run.py'))):
            if archived['files'][name]['sha256'] != g.sha(actual):
                raise ValueError('running product/lifecycle helper differs from gate archive')
        evidence = args.output / 'qualified-gate-evidence'
        provenance = restage_gate(g, args.gate_output, args.retained_gate, evidence)
        config = {'gate_controller': prior, 'gate_config': gate_config, 'container': args.container,
                  'execution': args.execution, 'execution_container_birth': {'id': before['id'], 'image': before['image'], 'created': before['created'], 'started_at': before['birth']},
                  'gate_root': str(Path(args.native_root) / 'qualified-gate-evidence'),
                  'gate_restaging': provenance, 'engine': g.load(args.engine),
                  'helper_sha256': g.sha(Path(g.__file__)),
                  'product_dir': '/opt/sail-m7-core-product-' + gate_config['revision'][:7],
                  'gate_inputs': {name: g.sha(evidence / name) for name in [*provenance['files'], 'restaging.json']}}
        g.save(args.output / 'product-config.json', config)
        shutil.copy2(archive, args.output / 'source.tar')
        command = ['docker', 'exec', '-i', args.container, 'python3', '-B', '-']
        call(command, payload=("from pathlib import Path\nPath(%r).mkdir(mode=0o700,exist_ok=False)\n" % args.native_root).encode())
        call(['docker', 'cp', str(args.output / 'product-config.json'), args.container + ':' + args.native_root + '/product-config.json'])
        call(['docker', 'cp', str(evidence), args.container + ':' + args.native_root + '/qualified-gate-evidence'])
        remote = gate_config['source'] + '/scripts/local-product-install.py'
        with (args.output / 'transport.log').open('wb') as log:
            result = call(['docker', 'exec', args.container, 'python3', '-B', remote, '--native', args.native_root],
                          timeout=BUILD_LIMIT + INSTALL_LIMIT + 180, output=log, check=False)
        after = g.identity(args.container, closure)
        report.update(container_before=before, container_after=after, transport_exit_code=result.returncode)
        if before != after:
            raise ValueError('container changed during product scope')
        call(['docker', 'cp', args.container + ':' + args.native_root + '/result.json', str(args.output / 'result.json')])
        terminal = g.load(args.output / 'result.json')
        if result.returncode != (0 if terminal['status'] == 'PASS' else 1):
            raise ValueError('native/transport terminal mismatch')
        for phase in terminal['phases']:
            name = phase['name'] + '.log'
            call(['docker', 'cp', args.container + ':' + args.native_root + '/' + name, str(args.output / name)])
            if g.sha(args.output / name) != phase['log_sha256']:
                raise ValueError('retained product log differs')
        if terminal['status'] == 'PASS':
            if terminal.get('uncertainty') or terminal.get('closure') != 'closed-recorded-scope' or [p['name'] for p in terminal['phases']] != ['release-build', 'owned-install'] or any(p.get('exit_code') != 0 for p in terminal['phases']):
                raise ValueError('incomplete native product PASS')
            call(['docker', 'cp', args.container + ':' + args.native_root + '/product-grant.json', str(args.output / 'product-grant.json')])
            if g.sha(args.output / 'product-grant.json') != terminal['product_grant_sha256']:
                raise ValueError('copied product grant differs')
        if g.sha(args.output / 'source.tar') != gate_config['archive_sha256']:
            raise ValueError('retained source archive differs')
        report['status'] = terminal['status']
    except BaseException as error:
        closure.uncertain('transport/setup closure requires separate audit')
        report.update(status='FAIL', error_type=type(error).__name__, error=str(error))
        if before is not None and remote is not None:
            try:
                if g.identity(args.container, closure) != before:
                    raise ValueError('container changed; no cancellation')
                call(['docker', 'exec', args.container, 'python3', '-B',
                      str(Path(remote).with_name('local-combined-gates.py')), '--cancel', args.native_root])
            except BaseException as error:
                report['cancellation_unavailable'] = str(error)
    finally:
        report.update(uncertainty=closure.reasons, native_root=args.native_root)
        g.save(args.output / 'controller.json', report)
    return 0 if report['status'] == 'PASS' and not closure.reasons else 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--native', type=Path, help=argparse.SUPPRESS)
    parser.add_argument('--gate-output', type=Path)
    parser.add_argument('--retained-gate', type=Path, help='exact verified completed-run retirement backing')
    parser.add_argument('--container')
    parser.add_argument('--native-root')
    parser.add_argument('--output', type=Path)
    parser.add_argument('--execution', help='explicit honest environment/load label retained in grant')
    parser.add_argument('--engine', type=Path, help='explicit exact13 runtime inventory + manifest SHA')
    args = parser.parse_args()
    if args.native:
        return native(args.native)
    if not all((args.gate_output, args.retained_gate, args.container, args.native_root, args.output, args.engine, args.execution)):
        parser.error('host mode requires gate-output/retained-gate/container/native-root/output/engine/execution')
    return host(args)


if __name__ == '__main__':
    raise SystemExit(main())
