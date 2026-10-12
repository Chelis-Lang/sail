#!/usr/bin/env python3
"""Build/seal a current harness, then run complete default standard comparisons.

Local only. Reuses archived lifecycle helpers; no image lifecycle or source edits.
"""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import shutil
import signal
import stat
import sys

sys.dont_write_bytecode = True
BUILD = ['cargo', 'build', '--release', '--locked', '-p', 'sail-conformance']
BUILD_LIMIT = 600
UNITS = 159
UNIT_LIMIT = 3600
# One initial plain, one Sail, at most two confirmation arms for every unit.
# This outer ownership cap does not shorten any original per-unit deadline.
ROUND_LIMIT = UNITS * 4 * UNIT_LIMIT + 300
ROUNDS = ('pass-through', 'cache-fill', 'cache-warm')


def module(path, name):
    spec = importlib.util.spec_from_file_location(name, path)
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


def need(condition, message):
    if not condition:
        raise ValueError(message)


def helpers(directory):
    directory = Path(directory)
    return (module(directory / 'local-combined-gates.py', 'standard_groups'),
            module(directory / 'local-product-install.py', 'standard_product'))


def command(harness, shim, output):
    return [str(harness), 'run', '--tier', 'standard', '--jobs', '1',
            '--shim', str(shim), '--out', str(output)]


def environment(home, cache):
    """Select default stock mode explicitly; preserve non-Sail environment."""
    result = {key: value for key, value in os.environ.items() if not key.startswith('SAIL_')}
    result.pop('CI', None)
    result.update(SAIL_VERUS_HOME=home, SAIL_CACHE_DIR=cache, SAIL_UNIT_TIMEOUT=str(UNIT_LIMIT))
    return result


def complete_report(value):
    """Keep the original classifier: Unstable remains a recorded distinct result."""
    summary = value['summary']
    need(value['schema_version'] == 1 and value['tier'] == 'standard', 'report kind differs')
    need(summary['units'] == UNITS and summary['projects_skipped'] == 0
         and value['skipped'] == [] and len(value['units']) == UNITS, 'whole corpus incomplete')
    names = [unit['id'] for unit in value['units']]
    need(len(set(names)) == UNITS, 'duplicate unit identity')
    need(summary['different'] == summary['infra_error'] == 0, 'original comparison failed')
    need(summary['same'] + summary['unstable'] == UNITS, 'classification count differs')
    return sorted(names)


def sequence(run, closure, harness, shim, root, start):
    for name in ROUNDS:
        closure.require()
        if name == 'cache-fill':
            start()
        if run(name, command(harness, shim, root / name), ROUND_LIMIT) != 0:
            return False
    closure.require()
    return True


def observing_groups(original, background):
    """Feed the same bounded phase census to the existing background primitive."""
    class PhaseGroups(original):
        def observe(self, rows):
            super().observe(rows)
            daemon = background()
            if daemon is not None:
                daemon.observe(rows)
    return PhaseGroups


def seal_harness(g, source, destination):
    """Copy the actual ELF once into a new private directory; never replace it."""
    info = source.lstat()
    need(stat.S_ISREG(info.st_mode) and 0 < info.st_size <= 128 * 1024 * 1024, 'harness size/type differs')
    with source.open('rb') as stream:
        need(stream.read(4) == b'\x7fELF', 'actual Linux ELF required')
    row = {'bytes': info.st_size, 'sha256': g.sha(source), 'mode': 0o700}
    destination.mkdir(mode=0o700, exist_ok=False)
    copied = destination / 'sail-conformance'
    shutil.copyfile(source, copied)
    copied.chmod(0o700)
    need(g.sha(copied) == row['sha256'] and g.sha(source) == row['sha256'], 'copied harness differs')
    row.update(uid=copied.stat().st_uid, gid=copied.stat().st_gid)
    return row


def native(root):
    config = json.loads((root / 'standard-config.json').read_text())
    support = root / 'support'
    g, product = helpers(support)
    grant, source = config['grant'], Path(config['grant']['source_path'])
    target = config['gate_config']['target']
    need(g.sha(Path(__file__)) == config['controller_sha256'], 'controller changed')
    for name, expected in config['helpers'].items():
        need(g.sha(support / name) == expected, 'archived helper changed')
    validator = module(support / 'm7-current-benchmark-controller.py', 'standard_grant')
    validator.validate_grant(grant, grant['source_commit'])
    holder_guard = module(support / 'inventory-old-benchmark-single-deps.py', 'standard_holders')
    g.verify_source(source, config['source_inventory'])
    g.save(root / 'config.json', dict(config['gate_config'], source=str(source)))
    who = g.support_module(root)
    owner = who.row(os.getpid())
    need(owner is not None, 'controller birth unavailable')
    g.save(root / 'owner.json', owner)
    closure = g.Closure()
    record = {'status': 'RUNNING', 'stage': config['stage'], 'phases': [], 'rounds': [],
              'source_commit': grant['source_commit'], 'source_archive_sha256': grant['source_archive_sha256'],
              'product_grant_sha256': config['grant_sha256'], 'jobs': 1, 'unit_limit_seconds': UNIT_LIMIT,
              'plain_runs': 1, 'confirmation_runs': 2, 'execution': config['execution'],
              'closure_scope': 'recorded owned phase/daemon groups only; no arbitrary orphan certificate'}
    harness_root = Path(config['harness_root'])
    harness = harness_root / 'sail-conformance'
    product_dir = Path(grant['product_dir'])
    stock = config['stock']
    def check():
        product.prerequisite(g, config['gate_controller'], config['gate_record'], config['gate_config'])
        g.verify_source(source, config['source_inventory'])
        source_info = source.lstat()
        need(source.resolve() == source and stat.S_ISDIR(source_info.st_mode)
             and stat.S_IMODE(source_info.st_mode) == 0o700 and source_info.st_uid == os.geteuid(),
             'source root owner/private binding differs')
        need(os.readlink(source / 'target') == target, 'source target changed')
        first = who.row(1)
        need(first is not None and first['start'] == config['gate_config']['pid1_birth'], 'PID1 birth changed')
        g.verify_protected(config['protected'])
        product.engine_seal(g, source, grant['selected_engine'])
        validator.native_seals(grant)
        g.verify_protected({'files': [dict(row, path=str(product_dir / name), mode=int(row['mode'], 8))
                                     for name, row in grant['installation']['files'].items()]})
        for name, expected in grant['installation']['files'].items():
            info = (product_dir / name).lstat()
            need((info.st_uid, info.st_gid) == (expected['uid'], expected['gid']), 'product file owner differs')
        info = product_dir.lstat()
        need(stat.S_ISDIR(info.st_mode) and stat.S_IMODE(info.st_mode) == 0o700
             and info.st_uid == grant['installation']['credentials']['fsuid'] == os.geteuid(), 'private product owner differs')
        g.verify_protected({'files': stock['files']})
        stock_root = Path(stock['home']) / 'verus-x86-linux'
        need(stock_root.resolve() == stock_root, 'stock runtime alias differs')
        need({str(p) for p in stock_root.iterdir() if p.is_file() or p.is_symlink()}
             == {row['path'] for row in stock['files']}, 'complete stock direct inventory differs')
        if config['stage'] == 'run':
            built = config['build_record']
            need(built['status'] == 'PASS' and built['closure'] == 'closed-recorded-scope'
                 and not built['uncertainty'] and built['product_grant_sha256'] == config['grant_sha256']
                 and built['source_commit'] == grant['source_commit']
                 and built['source_archive_sha256'] == grant['source_archive_sha256']
                 and len(built['phases']) == 1 and built['phases'][0]['command'] == BUILD
                 and built['phases'][0]['limit_seconds'] == BUILD_LIMIT
                 and built['phases'][0]['exit_code'] == 0
                 and g.closed_groups(built['phases'][0]['group_closure']),
                 'closed exact harness build required')
            g.verify_protected({'files': [dict(built['harness'], path=str(harness))]})
            info = harness.lstat()
            need((info.st_uid, info.st_gid) == (built['harness']['uid'], built['harness']['gid']), 'harness file owner differs')
            info = harness_root.lstat()
            need(stat.S_ISDIR(info.st_mode) and stat.S_IMODE(info.st_mode) == 0o700
                 and info.st_uid == built['harness']['uid'] == os.geteuid(), 'private harness owner differs')
    def audit_holders(name):
        rows = config['protected']['files'] + stock['files']
        identities = {(Path(row['path']).stat().st_dev, Path(row['path']).stat().st_ino) for row in rows}
        audit = holder_guard.holders([str(source), target, str(product_dir), str(harness_root)], identities, g.dead_threads)
        g.save(root / (name + '-holders.json'), audit)
        need(audit['complete'] is True and audit['holders'] == [] and audit['unavailable'] == []
             and len(audit['self_excluded']) == 1, 'scoped holder audit incomplete')
    daemons, daemon_groups = None, None
    original_groups = who.Groups
    who.Groups = observing_groups(original_groups, lambda: daemon_groups)
    runner = g.OwnedPhaseRunner(root, source, target, who, closure, record, check)
    for number in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
        signal.signal(number, runner.interrupt)
    previous_env = dict(os.environ)
    try:
        check()
        audit_holders('before')
        selected_environment = environment(stock['home'], config['corpus_cache'])
        record['removed_inherited_sail_names'] = sorted(key for key in os.environ if key.startswith('SAIL_'))
        os.environ.clear()
        os.environ.update(selected_environment)
        if config['stage'] == 'build':
            need(runner.run('release-build', BUILD, BUILD_LIMIT) == 0, 'ordinary harness build failed')
            record['harness'] = seal_harness(g, Path(target) / 'release/sail-conformance', harness_root)
            record['harness_root'] = str(harness_root)
        else:
            bench = module(source / 'scripts/m7-repair-bench.py', 'standard_daemons')
            daemons = bench.OwnedDaemons(root)
            socket = str(root / 'daemon.sock')
            cache = root / 'daemon-cache'
            def start():
                nonlocal daemon_groups
                env = dict(os.environ, SAIL_DAEMON_SOCKET=socket, SAIL_CACHE_DIR=str(cache))
                daemons.start('standard', product_dir / 'sail', source, env)
                actor = daemons.actors['standard'][0]
                birth = who.row(actor.pid)
                if birth is None or birth['pgid'] != actor.pid:
                    closure.uncertain('daemon birth unavailable')
                    raise ValueError('daemon birth unavailable')
                daemon_groups = original_groups(birth)
                record['daemon_owner'] = birth
                os.environ['SAIL_DAEMON_SOCKET'] = socket
            def run(name, argv, limit):
                if daemon_groups is not None:
                    g.observe_owned(who, daemon_groups)
                code = runner.run(name, argv, limit)
                if daemon_groups is not None:
                    g.observe_owned(who, daemon_groups)
                if code == 0:
                    report_path = root / name / 'report.json'
                    report = g.load(report_path)
                    names = complete_report(report)
                    if record['rounds']:
                        need(names == record['rounds'][0]['unit_ids'], 'round membership changed')
                    record['rounds'].append({'name': name, 'summary': report['summary'], 'unit_ids': names,
                                             'report_sha256': g.sha(report_path)})
                return code
            need(sequence(run, closure, harness, product_dir / 'sail-z3', root, start), 'standard round failed')
            count = 0
            for directory, _dirs, names in os.walk(cache / 'v2'):
                for name in names:
                    count += int((Path(directory) / name).is_file())
            need(count > 0, 'original cache population check failed')
            record['cache_entries'] = count
        record['status'] = 'PASS'
    except BaseException as error:
        record.update(status='FAIL', error_type=type(error).__name__, error=str(error))
    finally:
        g.save(root / 'primary.json', record)
        try:
            if daemons is not None:
                daemons.__exit__(None, None, None)
                if daemon_groups is None and daemons.actors:
                    closure.uncertain('daemon groups unavailable')
                if daemon_groups is not None:
                    audit = g.group_closure(who, daemon_groups)
                    record['daemon_group_closure'] = audit
                    record['daemon_known_groups'] = list(daemon_groups.known.values())
                    if not g.closed_groups(audit) or daemon_groups.partial:
                        closure.uncertain('daemon known-group closure unavailable')
            runner.require_idle()
            check()
            if config['stage'] == 'build' and 'harness' in record:
                g.verify_protected({'files': [dict(record['harness'], path=str(harness))]})
                compiled = Path(target) / 'release/sail-conformance'
                need(compiled.stat().st_size == record['harness']['bytes']
                     and g.sha(compiled) == record['harness']['sha256'], 'compiled harness changed after sealing')
            audit_holders('after')
            record['closure'] = 'closed-recorded-scope'
        except BaseException as error:
            record.update(status='FAIL', closure='unavailable', closure_error=str(error))
        os.environ.clear()
        os.environ.update(previous_env)
        record['uncertainty'] = closure.reasons
        g.save(root / 'result.json', record)
    return 0 if record['status'] == 'PASS' else 1


def host(args):
    g, product = helpers(Path(__file__).parent)
    closure = g.Closure()
    args.output.mkdir(mode=0o700, exist_ok=False)
    config = {'stage': args.stage, 'execution': args.execution}
    record = {'status': 'RUNNING', 'stage': args.stage}
    before, remote = None, None
    def interrupted(number, _frame):
        closure.uncertain('host signal ' + str(number))
        for value in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
            signal.signal(value, signal.SIG_IGN)
        raise InterruptedError('host controller interrupted')
    for number in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
        signal.signal(number, interrupted)
    def call(argv, **kwargs):
        return g.bounded_command(argv, closure=closure, **kwargs)
    try:
        need(args.execution and len(args.execution.encode()) <= 512, 'explicit bounded environment label required')
        gate = args.gate_output
        controller, terminal, gate_config = (g.load(gate / name) for name in ('controller.json', 'result.json', 'config.json'))
        product.prerequisite(g, controller, terminal, gate_config)
        grant = g.load(args.product_grant)
        validator = module(Path(__file__).parents[1] / 'docs/validation/m7-current-benchmark-controller.py', 'host_standard_grant')
        validator.validate_grant(grant, gate_config['revision'])
        need(grant['container'] == args.container and grant['source_archive_sha256'] == gate_config['archive_sha256']
             and grant['combined_gate_result_sha256'] == g.sha(gate / 'result.json'), 'producer/gate differs')
        before = g.identity(args.container, closure)
        need(before == controller['container_after']
             and before == dict(id=grant['execution_container_birth']['id'], image=grant['execution_container_birth']['image'],
                                birth=grant['execution_container_birth']['started_at'], created=grant['execution_container_birth']['created']),
             'current container differs from qualified producer')
        archive = gate / 'source.tar'
        need(g.sha(archive) == gate_config['archive_sha256'], 'source archive differs')
        inventory = g.inventory(archive)
        files = {'local-combined-gates.py': Path(g.__file__), 'local-product-install.py': Path(product.__file__),
                 'inventory-old-benchmark-single-deps.py': Path(__file__).with_name('inventory-old-benchmark-single-deps.py'),
                 'm7-current-benchmark-controller.py': Path(validator.__file__)}
        for name, path in files.items():
            relative = 'docs/validation/' + name if name.startswith('m7-current') else 'scripts/' + name
            need(inventory['files'][relative]['sha256'] == g.sha(path), 'helper differs from qualified source')
        stock = g.load(args.stock)
        need(stock['home'] == '/opt/verus' and isinstance(stock['files'], list) and stock['files'], 'explicit complete stock inventory required')
        names = {Path(row['path']).name for row in stock['files']}
        need({'verus', 'cargo-verus', 'rust_verify', 'z3'} <= names, 'stock executables missing')
        need(args.corpus_cache.startswith('/') and '..' not in Path(args.corpus_cache).parts, 'absolute corpus cache required')
        root = args.native_root
        need(root.startswith('/tmp/sail-local-standard-') and '..' not in Path(root).parts, 'fresh private native output required')
        config.update(grant=grant, grant_sha256=g.sha(args.product_grant), gate_config=gate_config,
                      gate_controller=controller, gate_record=terminal, source_inventory=inventory,
                      protected=g.load(gate / 'protected.json'), stock=stock, corpus_cache=args.corpus_cache,
                      harness_root='/opt/sail-m7-conformance-' + grant['source_commit'][:7],
                      controller_sha256=g.sha(Path(__file__)), helpers={name: g.sha(path) for name, path in files.items()})
        record.update(source_commit=grant['source_commit'], product_grant_sha256=config['grant_sha256'],
                      controller_sha256=config['controller_sha256'])
        if args.stage == 'run':
            need(args.build_output is not None, 'sealed separate build output required')
            build_controller = g.load(args.build_output / 'controller.json')
            need(build_controller['status'] == 'PASS' and not build_controller['uncertainty']
                 and build_controller['container_after'] == before, 'build transport incomplete/different birth')
            config['build_record'] = g.load(args.build_output / 'result.json')
        g.save(args.output / 'standard-config.json', config)
        call(['docker', 'exec', '-i', args.container, 'python3', '-B', '-'], timeout=30,
             payload=("from pathlib import Path\nr=Path(%r)\nr.mkdir(mode=0o700,exist_ok=False)\n(r/'support').mkdir(mode=0o700)\n" % root).encode())
        for path in [args.output / 'standard-config.json', Path(__file__)]:
            call(['docker', 'cp', str(path), args.container + ':' + root + '/' + path.name], timeout=30)
        for name, path in files.items():
            call(['docker', 'cp', str(path), args.container + ':' + root + '/support/' + name], timeout=30)
        remote = root + '/' + Path(__file__).name
        with (args.output / 'transport.log').open('wb') as log:
            result = call(['docker', 'exec', args.container, 'python3', '-B', remote, '--native', root],
                          timeout=BUILD_LIMIT + 180 if args.stage == 'build' else ROUND_LIMIT * 3 + 180,
                          output=log, check=False)
        after = g.identity(args.container, closure)
        need(before == after, 'container birth changed')
        call(['docker', 'cp', args.container + ':' + root + '/result.json', str(args.output / 'result.json')], timeout=30)
        terminal = g.load(args.output / 'result.json')
        need(result.returncode == (0 if terminal['status'] == 'PASS' else 1), 'native/transport mismatch')
        for phase in terminal['phases']:
            name = phase['name'] + '.log'
            call(['docker', 'cp', args.container + ':' + root + '/' + name, str(args.output / name)], timeout=60)
            need(g.sha(args.output / name) == phase['log_sha256'], 'phase log differs')
        for item in terminal['rounds']:
            name = item['name']
            call(['docker', 'cp', args.container + ':' + root + '/' + name + '/report.json',
                  str(args.output / (name + '-report.json'))], timeout=60)
            need(g.sha(args.output / (name + '-report.json')) == item['report_sha256'], 'retained full report differs')
        if args.stage == 'build' and terminal['status'] == 'PASS':
            call(['docker', 'cp', args.container + ':' + config['harness_root'] + '/sail-conformance', str(args.output / 'sail-conformance')], timeout=60)
            need(g.sha(args.output / 'sail-conformance') == terminal['harness']['sha256'], 'retained harness differs')
        record.update(status=terminal['status'], container_before=before, container_after=after,
                      result_sha256=g.sha(args.output / 'result.json'), transport_exit_code=result.returncode)
    except BaseException as error:
        closure.uncertain('host transport/setup requires independent closure audit')
        record.update(status='FAIL', error_type=type(error).__name__, error=str(error))
        if remote is not None and before is not None:
            try:
                need(g.identity(args.container, closure) == before, 'changed container; no cancellation')
                call(['docker', 'exec', args.container, 'python3', '-B', root + '/support/local-combined-gates.py', '--cancel', root], timeout=30)
            except BaseException as cleanup:
                record['cancellation_unavailable'] = str(cleanup)
    finally:
        record.update(uncertainty=closure.reasons, native_root=args.native_root)
        g.save(args.output / 'controller.json', record)
    return 0 if record['status'] == 'PASS' and not closure.reasons else 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--native', type=Path, help=argparse.SUPPRESS)
    parser.add_argument('--stage', choices=('build', 'run'))
    parser.add_argument('--gate-output', type=Path)
    parser.add_argument('--product-grant', type=Path)
    parser.add_argument('--build-output', type=Path)
    parser.add_argument('--stock', type=Path, help='complete direct stock release file inventory')
    parser.add_argument('--container')
    parser.add_argument('--native-root')
    parser.add_argument('--output', type=Path)
    parser.add_argument('--corpus-cache', default='/root/.cache/sail')
    parser.add_argument('--execution')
    args = parser.parse_args()
    if args.native:
        return native(args.native)
    if not all((args.stage, args.gate_output, args.product_grant, args.stock, args.container,
                args.native_root, args.output, args.execution)):
        parser.error('host mode requires stage/gate-output/product-grant/stock/container/native-root/output/execution')
    return host(args)


if __name__ == '__main__':
    raise SystemExit(main())
